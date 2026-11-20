// swift-tools-version:5.3

import PackageDescription

let package = Package(
  name: "tauri-plugin-ccid",
  platforms: [
    .macOS(.v10_13),
    .iOS(.v13),
  ],
  products: [
    .library(
      name: "tauri-plugin-ccid",
      type: .static,
      targets: ["tauri-plugin-ccid"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-ccid",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)
