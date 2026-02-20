// swift-tools-version:5.9

import PackageDescription

let package = Package(
    name: "tauri-plugin-llm",
    platforms: [
        .iOS(.v13)
    ],
    products: [
        .library(
            name: "tauri-plugin-llm",
            type: .static,
            targets: ["tauri-plugin-llm"]
        ),
    ],
    dependencies: [
        .package(name: "Tauri", path: "../.tauri/tauri-api")
    ],
    targets: [
        .target(
            name: "tauri-plugin-llm",
            dependencies: [
                .product(name: "Tauri", package: "Tauri")
            ],
            path: "Sources"
        ),
    ]
)
