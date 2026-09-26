import SwiftUI

struct TypeStyle { let tint: Color; let fg: Color; let symbol: String }

extension PrimaryKind {
    var style: TypeStyle {
        switch self {
        case .report: TypeStyle(tint: Sage.sageTint, fg: Sage.primary, symbol: "doc.text")
        case .message: TypeStyle(tint: Sage.sageTint, fg: Sage.primary, symbol: "bubble.left")
        case .appointment: TypeStyle(tint: Sage.sandTint, fg: Sage.sand, symbol: "calendar")
        case .alert: TypeStyle(tint: Sage.clayTint, fg: Sage.clay, symbol: "bell")
        }
    }
}

struct TypeBadge: View {
    let type: PrimaryKind
    var body: some View {
        Text(type.label).font(HFont.tiny).foregroundStyle(type.style.fg)
            .padding(.horizontal, 8).padding(.vertical, 2)
            .background(type.style.tint, in: RoundedRectangle(cornerRadius: 10))
    }
}

struct TypeTile: View {
    let type: PrimaryKind
    var size: CGFloat = 40
    var body: some View {
        Image(systemName: type.style.symbol).font(.system(size: size * 0.45, weight: .regular))
            .foregroundStyle(type.style.fg)
            .frame(width: size, height: size)
            .background(type.style.tint, in: RoundedRectangle(cornerRadius: Radius.tile))
            .accessibilityHidden(true)
    }
}

struct DataItemRow: View {
    let item: DataItem
    var body: some View {
        HStack(spacing: 12) {
            TypeTile(type: item.primaryKind)
            VStack(alignment: .leading, spacing: 2) {
                Text(item.title).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                Text(item.subtitle).font(HFont.caption).foregroundStyle(Sage.muted).lineLimit(1)
            }
            Spacer(minLength: 8)
            VStack(alignment: .trailing, spacing: 4) {
                Text(DateText.short(item.date)).font(HFont.small).foregroundStyle(Sage.muted)
                TypeBadge(type: item.primaryKind)
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }
}

struct Avatar: View {
    let initials: String
    var size: CGFloat = 44
    var background: Color = Sage.avatar
    var ring: Color? = nil
    var body: some View {
        Text(initials).font(HFont.display(size * 0.34)).foregroundStyle(Sage.primary)
            .frame(width: size, height: size)
            .background(background, in: Circle())
            .overlay { if let ring { Circle().stroke(ring, lineWidth: 2) } }
            .accessibilityHidden(true)
    }
}

struct SectionHeader: View {
    let title: String
    var action: String? = nil
    var onAction: () -> Void = {}
    var body: some View {
        HStack {
            Text(title).font(HFont.section).foregroundStyle(Sage.ink)
            Spacer()
            if let action { Button(action, action: onAction).font(HFont.bodyStrong).foregroundStyle(Sage.primary) }
        }
    }
}

struct FieldLabel: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View { Text(text).font(HFont.captionStrong).foregroundStyle(Sage.inkSoft) }
}

struct SageChip: View {
    let label: String
    let selected: Bool
    var onHeader = false
    let action: () -> Void
    var body: some View {
        let (bg, fg, border): (Color, Color, Color) = switch (onHeader, selected) {
        case (true, true): (.white, Sage.primary, .white)
        case (true, false): (Sage.primaryRaised, .white, Sage.primaryRaised)
        case (false, true): (Sage.primary, .white, Sage.primary)
        case (false, false): (Sage.surface, Sage.ink, Sage.border)
        }
        Button(action: action) {
            Text(label).font(HFont.captionStrong).foregroundStyle(fg)
                .padding(.horizontal, 14).frame(minHeight: 40)
                .background(bg, in: Capsule())
                .overlay(Capsule().stroke(border, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// Two-option segmented control used for tabs and Open/Closed.
struct Segmented: View {
    let options: [String]
    @Binding var selection: Int
    var body: some View {
        HStack(spacing: 0) {
            ForEach(options.indices, id: \.self) { i in
                let on = i == selection
                Button { selection = i } label: {
                    Text(options[i]).font(on ? HFont.bodyStrong : HFont.body).foregroundStyle(on ? Sage.ink : Sage.muted)
                        .frame(maxWidth: .infinity, minHeight: 40)
                        .background(on ? Sage.surface : .clear, in: RoundedRectangle(cornerRadius: 11))
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
        .padding(3)
        .background(Sage.sunken, in: RoundedRectangle(cornerRadius: Radius.field))
    }
}

struct HeaderIconButton: View {
    let symbol: String
    let label: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 17, weight: .semibold)).foregroundStyle(.white)
                .frame(width: 44, height: 44).background(Sage.primaryRaised, in: Circle())
        }
        .accessibilityLabel(label)
    }
}

/// 10% pinned header used by Data view and Upload.
struct PinnedHeader<Trailing: View>: View {
    let title: String
    let subtitle: String
    var backSymbol = "chevron.left"
    var backLabel = "Back"
    let onBack: () -> Void
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(spacing: 12) {
            HeaderIconButton(symbol: backSymbol, label: backLabel, action: onBack)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(HFont.headerTitle).foregroundStyle(.white).lineLimit(1)
                Text(subtitle).font(HFont.small).foregroundStyle(Sage.onPrimarySoft).lineLimit(1)
            }
            Spacer(minLength: 0)
            trailing()
        }
        .padding(.horizontal, 16)
        .heightFraction(HeaderRatio.pinned)
        .frame(maxWidth: .infinity)
        .background(Sage.primary.ignoresSafeArea(edges: .top))
    }
}

struct PrimaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(HFont.bodyStrong).foregroundStyle(.white)
            .frame(maxWidth: .infinity, minHeight: 50)
            .background(configuration.isPressed ? Sage.primaryPressed : Sage.primary, in: Capsule())
    }
}

struct SecondaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(HFont.bodyStrong).foregroundStyle(Sage.primary)
            .frame(maxWidth: .infinity, minHeight: 50)
            .background(configuration.isPressed ? Sage.sageTint : Sage.surface, in: Capsule())
            .overlay(Capsule().stroke(Sage.primary, lineWidth: 1))
    }
}

struct GroupCard<Content: View>: View {
    @ViewBuilder var content: () -> Content
    var body: some View {
        VStack(spacing: 0, content: content)
            .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
    }
}

struct RowDivider: View {
    var body: some View { Rectangle().fill(Sage.rowDivider).frame(height: 1).padding(.leading, 14) }
}

struct LoadingView: View {
    var body: some View { ProgressView().tint(Sage.primary).frame(maxWidth: .infinity).padding(32) }
}

struct ErrorView: View {
    let message: String
    let retry: () -> Void
    var body: some View {
        VStack(spacing: 10) {
            Text(message).font(HFont.body).foregroundStyle(Sage.ink).multilineTextAlignment(.center)
            Button("Try again", action: retry).buttonStyle(SecondaryButtonStyle()).frame(maxWidth: 200)
        }
        .padding(24).frame(maxWidth: .infinity)
    }
}

extension View {
    /// White action bar pinned above the home indicator.
    func bottomActionBar<Bar: View>(@ViewBuilder _ bar: () -> Bar) -> some View {
        safeAreaInset(edge: .bottom, spacing: 0) {
            VStack(spacing: 0) {
                Rectangle().fill(Sage.divider).frame(height: 1)
                HStack(spacing: 10, content: bar).padding(.horizontal, 20).padding(.vertical, 12)
            }
            .background(Sage.surface.ignoresSafeArea(edges: .bottom))
        }
    }

    /// Sage header block with rounded bottom corners (Landing, Search, Settings, Messages).
    func sageHeaderBackground(rounded: Bool = true) -> some View {
        background(
            UnevenRoundedRectangle(bottomLeadingRadius: rounded ? Radius.header : 0, bottomTrailingRadius: rounded ? Radius.header : 0)
                .fill(Sage.primary).ignoresSafeArea(edges: .top)
        )
    }
}
