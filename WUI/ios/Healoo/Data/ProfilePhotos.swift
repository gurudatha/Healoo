import UIKit

/// Profile pictures are resized on the phone before upload: at most `maxPixels` on the long side, JPEG.
enum ProfilePhotos {
    static let maxPixels: CGFloat = 512

    /// Scales the picked or captured image down and writes a JPEG into the caches folder, ready to upload.
    static func prepare(_ image: UIImage) throws -> PendingAttachment {
        let scale = min(1, maxPixels / max(image.size.width, image.size.height))
        let size = CGSize(width: max(1, (image.size.width * scale).rounded()), height: max(1, (image.size.height * scale).rounded()))
        let format = UIGraphicsImageRendererFormat.default()
        format.scale = 1   // size is in pixels
        // Drawing also applies the camera's orientation.
        let resized = UIGraphicsImageRenderer(size: size, format: format).image { _ in image.draw(in: CGRect(origin: .zero, size: size)) }
        guard let data = resized.jpegData(compressionQuality: 0.85) else { throw RepoError("Couldn't prepare the photo") }
        let dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("profile", isDirectory: true)
        try? FileManager.default.removeItem(at: dir)   // older prepared photos are no longer needed
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent("photo-\(Int(Date().timeIntervalSince1970)).jpg")
        try data.write(to: url)
        return PendingAttachment(localURL: url, kind: .image, name: "profile.jpg", mime: "image/jpeg", size: Int64(data.count))
    }
}
