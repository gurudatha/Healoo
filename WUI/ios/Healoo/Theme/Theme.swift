import SwiftUI

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255,
                  opacity: 1)
    }
}

/// Sage Clinic tokens — mirrors design-tokens.json. Theme colour #AAB5AD (grey-green).
///
/// Primary is light, so the roles are split (same as Android's Theme.kt):
/// - `primary` / `primaryRaised` are surfaces (headers, buttons, selected chips, tiles), with
///   `onPrimary` / `onPrimarySoft` text and icons on them (white on #AAB5AD would be ~2:1 contrast).
/// - `accent`, a darker shade of the same colour, is for links, icons and text on light backgrounds.
enum Sage {
    static let primary = Color(hex: 0xAAB5AD)
    static let primaryRaised = Color(hex: 0xC3CBC5)
    static let primaryPressed = Color(hex: 0x97A39A)
    static let onPrimary = Color(hex: 0x1F2A27)
    static let onPrimarySoft = Color(hex: 0x34403A)
    static let onPrimaryLine = Color(hex: 0x7F8C84)
    static let accent = Color(hex: 0x4B5A51)
    /// Background of closed items (open ones are white).
    static let closed = Color(hex: 0xE3E4E2)

    static let background = Color(hex: 0xF5F3EE)
    static let surface = Color.white
    static let sunken = Color(hex: 0xE9E5DC)
    static let preview = Color(hex: 0xEDEAE2)

    static let ink = Color(hex: 0x1F2A27)
    static let inkSoft = Color(hex: 0x3E4A46)
    static let muted = Color(hex: 0x56625E)
    static let placeholder = Color(hex: 0x66706C)

    static let border = Color(hex: 0xD9D5CB)
    static let divider = Color(hex: 0xE4E0D6)
    static let rowDivider = Color(hex: 0xEEEBE4)

    static let sageTint = Color(hex: 0xE4E9E5)
    static let avatar = Color(hex: 0xE6EBE7)
    static let clay = Color(hex: 0x9A4A26)
    static let clayTint = Color(hex: 0xF6E6DC)
    static let clayBorder = Color(hex: 0xE8CFC0)
    static let sand = Color(hex: 0x5B5340)
    static let sandTint = Color(hex: 0xEFEADF)
    static let sandInk = Color(hex: 0x4A4436)
    static let switchOff = Color(hex: 0xCFCAC0)
    static let dashed = Color(hex: 0xB8C1BA)
    static let viewerBackground = Color(hex: 0x141A18)
}

enum Radius {
    static let header: CGFloat = 28
    static let card: CGFloat = 16
    static let tile: CGFloat = 12
    static let field: CGFloat = 14
    static let chip: CGFloat = 20
    static let pill: CGFloat = 25
}

/// Header heights as a fraction of screen height (design doc 3.2 / 3.3).
enum HeaderRatio {
    static let landing: CGFloat = 0.20
    static let userExpanded: CGFloat = 0.25
    static let userCollapsed: CGFloat = 0.10
    static let pinned: CGFloat = 0.10
}

/// PostScript names of the bundled fonts. If a name doesn't match the file you added,
/// SwiftUI silently falls back to the system font — check names in Font Book.
enum FontName {
    static let fraunces = "Fraunces-SemiBold"
    static let figtreeRegular = "Figtree-Regular"
    static let figtreeMedium = "Figtree-Medium"
    static let figtreeSemiBold = "Figtree-SemiBold"
}

enum HFont {
    static func display(_ size: CGFloat, relativeTo style: Font.TextStyle = .title2) -> Font {
        .custom(FontName.fraunces, size: size, relativeTo: style)
    }
    static let screenTitle = display(26, relativeTo: .largeTitle)
    static let greeting = display(24, relativeTo: .title)
    static let profileName = display(23, relativeTo: .title)
    static let headerTitle = display(18, relativeTo: .headline)
    static let section = display(19, relativeTo: .title3)
    static let counter = display(26, relativeTo: .title)

    static let body = Font.custom(FontName.figtreeRegular, size: 15, relativeTo: .body)
    static let bodyMedium = Font.custom(FontName.figtreeMedium, size: 15, relativeTo: .body)
    static let bodyStrong = Font.custom(FontName.figtreeSemiBold, size: 15, relativeTo: .body)
    static let caption = Font.custom(FontName.figtreeRegular, size: 13, relativeTo: .subheadline)
    static let captionStrong = Font.custom(FontName.figtreeSemiBold, size: 13, relativeTo: .subheadline)
    static let small = Font.custom(FontName.figtreeRegular, size: 12, relativeTo: .footnote)
    static let smallStrong = Font.custom(FontName.figtreeSemiBold, size: 12, relativeTo: .footnote)
    static let tiny = Font.custom(FontName.figtreeSemiBold, size: 11, relativeTo: .caption2)
}

extension View {
    /// Height as a fraction of the nearest container (the screen for top-level views).
    func heightFraction(_ fraction: CGFloat) -> some View {
        containerRelativeFrame(.vertical) { length, _ in length * fraction }
    }
}
