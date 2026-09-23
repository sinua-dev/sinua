import SwiftUI
import UIKit
import Sinua

final class AssistantViewController: UIViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        // Host the SwiftUI view; it pauses off-screen and in the background by itself.
        let host = UIHostingController(rootView: SinuaOrb(pattern: .listening))
        host.view.backgroundColor = .clear
        addChild(host)
        view.addSubview(host.view)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            host.view.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            host.view.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            host.view.widthAnchor.constraint(equalToConstant: 160),
            host.view.heightAnchor.constraint(equalToConstant: 160),
        ])
        host.didMove(toParent: self)
    }
}
