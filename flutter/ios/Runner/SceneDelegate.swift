import UIKit
import Flutter

@objc class SceneDelegate: FlutterSceneDelegate {
  override func scene(_ scene: UIScene, willConnectTo session: UISceneSession,
                      options connectionOptions: UIScene.ConnectionOptions) {
    super.scene(scene, willConnectTo: session, options: connectionOptions)
    if let view = window?.rootViewController?.view {
      (UIApplication.shared.delegate as? AppDelegate)?.windowGeometry?.attach(view: view)
    }
  }

  override func sceneDidBecomeActive(_ scene: UIScene) {
    super.sceneDidBecomeActive(scene)
    (UIApplication.shared.delegate as? AppDelegate)?.windowGeometry?.refresh()
  }
}
