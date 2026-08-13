import SwiftUI

struct NoemaBootView: View {
  @State private var selection = NoemaDestination.chat

  var body: some View {
    GeometryReader { proxy in
      let compact = NoemaBreakpoint.resolve(width: proxy.size.width) == .compact
      let safeTop = proxy.safeAreaInsets.top
      let deckTop = safeTop + 52

      ZStack(alignment: .topLeading) {
        NoemaColor.pine50.ignoresSafeArea()

        VStack(spacing: 0) {
          ChatTranscriptLoadingSkeleton()
          bootComposer
            .padding(.horizontal, NoemaSpacing.xl)
            .padding(.bottom, proxy.safeAreaInsets.bottom + NoemaSpacing.sm)
        }
        .frame(
          width: proxy.size.width,
          height: proxy.size.height + safeTop + proxy.safeAreaInsets.bottom - deckTop
        )
        .background(NoemaColor.surface)
        .clipShape(
          NoemaSuperellipse(
            topLeftRadius: NoemaRadius.page,
            topRightRadius: NoemaRadius.page,
            bottomRightRadius: compact ? 0 : NoemaRadius.page,
            bottomLeftRadius: compact ? 0 : NoemaRadius.page,
            treatment: .page
          )
        )
        .offset(y: deckTop)
        .shadow(color: NoemaColor.pine500.opacity(0.08), radius: 8)

        NoemaTopRail(
          selection: $selection,
          breakpoint: NoemaBreakpoint.resolve(width: proxy.size.width),
          agentLabel: "Chat",
          selectionPosition: 0
        )
        .padding(.top, safeTop)
        .allowsHitTesting(false)
      }
      .ignoresSafeArea()
    }
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Preparing Noema")
  }

  private var bootComposer: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Text("Send a message")
        .font(NoemaFont.composer)
        .foregroundStyle(NoemaColor.white.opacity(0.72))
        .padding(.leading, NoemaSpacing.lg)
      Spacer(minLength: NoemaSpacing.md)
      Image(systemName: "paperplane")
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.pine500.opacity(0.7))
        .frame(width: 40, height: 40)
        .background(NoemaColor.white, in: NoemaSuperellipse(cornerRadius: 26))
        .padding(.trailing, NoemaSpacing.xs)
    }
    .frame(maxWidth: 760, minHeight: 50, alignment: .trailing)
    .background(NoemaColor.pine500, in: NoemaSuperellipse(cornerRadius: 26))
    .shadow(color: NoemaColor.pine700.opacity(0.10), radius: 12, y: 5)
    .frame(maxWidth: .infinity, alignment: .trailing)
    .accessibilityHidden(true)
  }
}
