import AVFoundation
import Accelerate

class AudioRecorder {
    private var audioEngine: AVAudioEngine?
    private var audioFile: AVAudioFile?
    private var currentURL: URL?
    private var startTime: Date?

    /// Real spectrum data — 32 frequency bands, updated in real-time
    private(set) var spectrum: [Float] = Array(repeating: 0, count: 32)
    private var smoothedSpectrum: [Float] = Array(repeating: 0, count: 32)
    private let spectrumLock = NSLock()
    private let fftQueue = DispatchQueue(label: "com.local.WhisperDictate.fft", qos: .userInteractive)

    func startRecording() {
        let tempDir = FileManager.default.temporaryDirectory
        let filename = "whisper_dictate_\(ProcessInfo.processInfo.globallyUniqueString).wav"
        let url = tempDir.appendingPathComponent(filename)
        currentURL = url
        startTime = Date()

        let engine = AVAudioEngine()
        let inputNode = engine.inputNode
        let recordingFormat = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: 16000,
            channels: 1,
            interleaved: false
        )!

        // Convert input format to our recording format
        let inputFormat = inputNode.outputFormat(forBus: 0)

        do {
            // Create output file in 16-bit PCM WAV for whisper
            let wavFormat = AVAudioFormat(
                commonFormat: .pcmFormatInt16,
                sampleRate: 16000,
                channels: 1,
                interleaved: false
            )!
            audioFile = try AVAudioFile(forWriting: url, settings: wavFormat.settings)
        } catch {
            Log.error("Failed to create audio file: \(error)")
            return
        }

        // Install tap on input node
        let converter = AVAudioConverter(from: inputFormat, to: recordingFormat)

        inputNode.installTap(onBus: 0, bufferSize: 1024, format: inputFormat) { [weak self] buffer, _ in
            guard let self = self else { return }

            // Convert to our format
            let frameCount = AVAudioFrameCount(1024)
            guard let convertedBuffer = AVAudioPCMBuffer(pcmFormat: recordingFormat, frameCapacity: frameCount) else { return }

            var error: NSError?
            let status = converter?.convert(to: convertedBuffer, error: &error) { inNumPackets, outStatus in
                outStatus.pointee = .haveData
                return buffer
            }

            if status == .haveData || status == .inputRanDry {
                // Write to file
                do {
                    try self.audioFile?.write(from: convertedBuffer)
                } catch {
                    // Ignore write errors during recording
                }

                // Compute FFT off the audio thread
                self.fftQueue.async { self.computeSpectrum(buffer: convertedBuffer) }
            }
        }

        do {
            engine.prepare()
            try engine.start()
            audioEngine = engine

            let inputDeviceName = AVCaptureDevice.default(for: .audio)?.localizedName ?? "unknown"
            Log.info("Recording started → \(url.lastPathComponent) (mic: \(inputDeviceName))")
        } catch {
            Log.error("Failed to start audio engine: \(error)")
        }
    }

    /// Current audio level in dB (legacy, for compatibility)
    func currentLevel() -> Float {
        let s = spectrumLock.withLock { spectrum }
        let maxVal = s.max() ?? 0
        if maxVal <= 0 { return -160 }
        return 20 * log10(maxVal)
    }

    /// Get smoothed spectrum for visualization
    func getSpectrum() -> [Float] {
        spectrumLock.withLock { smoothedSpectrum }
    }

    func stopRecording() -> URL? {
        guard let engine = audioEngine else { return nil }
        let duration = -(startTime?.timeIntervalSinceNow ?? 0)

        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        audioEngine = nil
        audioFile = nil

        Log.info("Recording stopped (\(String(format: "%.1f", duration))s)")

        if let url = currentURL, let attrs = try? FileManager.default.attributesOfItem(atPath: url.path) {
            let size = (attrs[.size] as? Int) ?? 0
            Log.info("Audio file size: \(size) bytes")
        }

        guard duration >= 0.3 else {
            Log.info("Recording too short, ignoring")
            if let url = currentURL {
                try? FileManager.default.removeItem(at: url)
            }
            return nil
        }

        return currentURL
    }

    // MARK: - FFT

    private func computeSpectrum(buffer: AVAudioPCMBuffer) {
        guard let channelData = buffer.floatChannelData?[0] else { return }
        let frameCount = Int(buffer.frameLength)
        guard frameCount > 0 else { return }

        // Use power of 2 for FFT
        let log2n = vDSP_Length(10) // 1024 samples
        let n = 1 << Int(log2n)
        let fftSize = min(frameCount, n)

        guard let fftSetup = vDSP_create_fftsetup(log2n, FFTRadix(kFFTRadix2)) else { return }
        defer { vDSP_destroy_fftsetup(fftSetup) }

        // Prepare input with windowing
        var windowedSignal = [Float](repeating: 0, count: n)
        var window = [Float](repeating: 0, count: n)
        vDSP_hann_window(&window, vDSP_Length(n), Int32(vDSP_HANN_NORM))

        for i in 0..<fftSize {
            windowedSignal[i] = channelData[i] * window[i]
        }

        // Split complex
        var realp = [Float](repeating: 0, count: n / 2)
        var imagp = [Float](repeating: 0, count: n / 2)

        realp.withUnsafeMutableBufferPointer { realBuf in
            imagp.withUnsafeMutableBufferPointer { imagBuf in
                var splitComplex = DSPSplitComplex(
                    realp: realBuf.baseAddress!,
                    imagp: imagBuf.baseAddress!
                )
                windowedSignal.withUnsafeBufferPointer { signalBuf in
                    signalBuf.baseAddress!.withMemoryRebound(to: DSPComplex.self, capacity: n / 2) { ptr in
                        vDSP_ctoz(ptr, 2, &splitComplex, 1, vDSP_Length(n / 2))
                    }
                }
                vDSP_fft_zrip(fftSetup, &splitComplex, 1, log2n, FFTDirection(FFT_FORWARD))

                // Compute magnitudes
                var magnitudes = [Float](repeating: 0, count: n / 2)
                vDSP_zvmags(&splitComplex, 1, &magnitudes, 1, vDSP_Length(n / 2))

                // Group into 32 bands (logarithmic spacing)
                let bandCount = 32
                let binCount = n / 2
                var bands = [Float](repeating: 0, count: bandCount)

                for band in 0..<bandCount {
                    let lowFrac = Float(band) / Float(bandCount)
                    let highFrac = Float(band + 1) / Float(bandCount)
                    let lowBin = Int(pow(lowFrac, 2.0) * Float(binCount))
                    let highBin = max(lowBin + 1, Int(pow(highFrac, 2.0) * Float(binCount)))

                    var sum: Float = 0
                    let clampedHigh = min(highBin, binCount)
                    for bin in lowBin..<clampedHigh {
                        sum += magnitudes[bin]
                    }
                    bands[band] = sum / Float(clampedHigh - lowBin)
                }

                // Fixed reference level — low enough to show voice clearly
                let referenceLevel: Float = 5.0

                self.spectrumLock.withLock {
                    for i in 0..<bandCount {
                        let raw = sqrt(bands[i]) / referenceLevel
                        let clamped = min(1.0, max(0.0, raw))
                        let gated: Float = clamped < 0.03 ? 0.0 : clamped
                        self.smoothedSpectrum[i] = self.smoothedSpectrum[i] * 0.3 + gated * 0.7
                    }
                    self.spectrum = self.smoothedSpectrum
                }
            }
        }
    }
}
