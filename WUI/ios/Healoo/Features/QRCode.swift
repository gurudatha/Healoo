import SwiftUI
import VisionKit
import CoreImage.CIFilterBuiltins

enum HealooQR {
    static func content(for publicId: String) -> String { "healoo://id/\(publicId)" }

    /// Accepts "healoo://id/HL-2M9P4" or a bare "HL-2M9P4".
    static func parse(_ raw: String) -> String? {
        guard let r = raw.uppercased().range(of: #"HL-[A-Z0-9]{5}"#, options: .regularExpression) else { return nil }
        return String(raw.uppercased()[r])
    }

    static func image(for content: String) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(content.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage?.transformed(by: CGAffineTransform(scaleX: 12, y: 12)),
              let cg = CIContext().createCGImage(output, from: output.extent) else { return nil }
        return UIImage(cgImage: cg)
    }
}

struct QRCodeView: View {
    let content: String
    var body: some View {
        if let img = HealooQR.image(for: content) {
            Image(uiImage: img).interpolation(.none).resizable().scaledToFit()
        } else {
            Image(systemName: "qrcode").resizable().scaledToFit().foregroundStyle(Sage.muted)
        }
    }
}

/// Full-screen QR scanner. Calls `onResult` with the Healoo ID, or nil if the user cancels.
struct QRScannerSheet: View {
    let onResult: (String?) -> Void
    @State private var note: String?

    var body: some View {
        ZStack(alignment: .top) {
            if DataScannerViewController.isSupported && DataScannerViewController.isAvailable {
                ScannerRepresentable { raw in
                    if let id = HealooQR.parse(raw) { onResult(id) } else { note = "That QR code isn't a Healoo ID." }
                }
                .ignoresSafeArea()
            } else {
                VStack(spacing: 12) {
                    Image(systemName: "camera.fill").font(.largeTitle).foregroundStyle(.white)
                    Text("Scanning needs a camera and camera access. Type the Healoo ID in Search instead, or allow camera access in Settings.")
                        .font(HFont.body).foregroundStyle(.white).multilineTextAlignment(.center)
                }
                .padding(32).frame(maxWidth: .infinity, maxHeight: .infinity).background(Sage.viewerBackground)
            }
            HStack {
                Button { onResult(nil) } label: {
                    Image(systemName: "xmark").font(.system(size: 17, weight: .semibold)).foregroundStyle(.white)
                        .frame(width: 44, height: 44).background(.black.opacity(0.4), in: Circle())
                }
                .accessibilityLabel("Close scanner")
                Spacer()
            }
            .padding(16)
            VStack {
                Spacer()
                Text(note ?? "Point the camera at a Healoo ID QR code")
                    .font(HFont.bodyStrong).foregroundStyle(.white).padding(12)
                    .background(.black.opacity(0.5), in: Capsule()).padding(.bottom, 40)
            }
        }
    }
}

private struct ScannerRepresentable: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let vc = DataScannerViewController(recognizedDataTypes: [.barcode(symbologies: [.qr])],
                                           qualityLevel: .balanced, isHighlightingEnabled: true)
        vc.delegate = context.coordinator
        try? vc.startScanning()
        return vc
    }

    func updateUIViewController(_ vc: DataScannerViewController, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator(onCode: onCode) }

    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        let onCode: (String) -> Void
        private var done = false
        init(onCode: @escaping (String) -> Void) { self.onCode = onCode }

        func dataScanner(_ scanner: DataScannerViewController, didAdd addedItems: [RecognizedItem], allItems: [RecognizedItem]) {
            guard !done else { return }
            for item in addedItems {
                if case .barcode(let code) = item, let raw = code.payloadStringValue {
                    if HealooQR.parse(raw) != nil { done = true; scanner.stopScanning() }
                    onCode(raw)
                    return
                }
            }
        }
    }
}
