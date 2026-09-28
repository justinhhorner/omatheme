import Foundation
import Observation
import OmarchyThemesStores
import SwiftUI

/// The app's composition root: the stores (OmarchyThemesStores, where the logic and its tests
/// live), the thumbnail loader and sidebar navigation. Views take the individual stores they use
/// from the environment (see `appEnvironment(_:)`).
@MainActor
@Observable
final class AppModel {
    let stores: AppStores
    let images: ImageLoader
    var sidebarSelection: SidebarItem? = .gallery

    init(services: AppServices = .live()) {
        stores = AppStores(services: services)
        images = ImageLoader(cacheDirectory: services.paths.cacheDir.appending(path: "images", directoryHint: .isDirectory))
    }

    /// Clears the HTTP cache, resolved themes and thumbnails. Downloaded themes are kept.
    func clearCache() throws {
        try stores.clearCache()
        images.clearMemory()
    }
}

enum SidebarItem: Hashable {
    case gallery
    case downloaded
}

extension View {
    /// Puts the model and each of its stores in the environment.
    func appEnvironment(_ model: AppModel) -> some View {
        environment(model)
            .environment(model.stores.banners)
            .environment(model.stores.preferences)
            .environment(model.stores.catalog)
            .environment(model.stores.library)
            .environment(model.stores.desktop)
            .environment(model.stores.terminals)
            .environment(\.imageLoader, model.images)
    }
}
