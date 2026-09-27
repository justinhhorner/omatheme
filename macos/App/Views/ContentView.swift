import OmarchyThemesKit
import SwiftUI

/// The main window: sidebar (Gallery, Downloaded) and a drill-in stack for each.
struct ContentView: View {
    @Environment(AppModel.self) private var model
    @State private var galleryPath: [CatalogEntry] = []
    @State private var downloadedPath: [CatalogEntry] = []

    var body: some View {
        @Bindable var model = model

        NavigationSplitView {
            List(selection: $model.sidebarSelection) {
                Section("Themes") {
                    Label("Gallery", systemImage: "square.grid.2x2")
                        .tag(SidebarItem.gallery)
                        .accessibilityIdentifier("sidebar.gallery")
                    Label("Downloaded", systemImage: "arrow.down.circle")
                        .badge(model.installed.count)
                        .tag(SidebarItem.downloaded)
                        .accessibilityIdentifier("sidebar.downloaded")
                }
            }
            .navigationSplitViewColumnWidth(min: 170, ideal: 200, max: 280)
        } detail: {
            switch model.sidebarSelection ?? .gallery {
            case .gallery:
                NavigationStack(path: $galleryPath) {
                    GalleryView()
                        .navigationDestination(for: CatalogEntry.self) { ThemeDetailView(entry: $0) }
                }
            case .downloaded:
                NavigationStack(path: $downloadedPath) {
                    DownloadedView { downloadedPath.append($0) }
                        .navigationDestination(for: CatalogEntry.self) { ThemeDetailView(entry: $0) }
                }
            }
        }
        .sheet(isPresented: $model.showWelcome, onDismiss: { model.dismissWelcome() }) {
            WelcomeView()
        }
        .overlay(alignment: .bottom) {
            if model.isDryRun {
                Text("Dry run: applying a theme won't change your desktop")
                    .font(.caption.weight(.medium))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 4)
                    .background(.orange.opacity(0.9), in: Capsule())
                    .foregroundStyle(.white)
                    .padding(8)
                    .allowsHitTesting(false)
            }
        }
    }
}
