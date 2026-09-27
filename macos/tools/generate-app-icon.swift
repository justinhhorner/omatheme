#!/usr/bin/env swift
// Renders design/AppIcon.svg into the macOS app icon set:
//
//     swift macos/tools/generate-app-icon.swift
//
// The SVG's rounded square fills 896/1024 of its canvas (the Windows look). macOS icons sit on
// Apple's icon grid instead: an 824 pt body centred in 1024 with a soft drop shadow, so the SVG
// is scaled down onto that grid rather than drawn full-bleed.

import AppKit

let root = URL(filePath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
let svg = root.appending(path: "design/AppIcon.svg")
let output = root.appending(path: "macos/App/Assets.xcassets/AppIcon.appiconset")

guard let source = NSImage(contentsOf: svg) else {
    fatalError("Couldn't read \(svg.path)")
}

/// Size of the SVG's rounded square relative to its canvas, and the macOS grid's body size.
let svgBody = 896.0 / 1024.0
let macBody = 824.0 / 1024.0

func render(pixels: Int) -> Data {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: pixels, height: pixels)

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current?.imageInterpolation = .high

    let canvas = CGFloat(pixels)
    let side = canvas * macBody / svgBody
    let rect = NSRect(x: (canvas - side) / 2, y: (canvas - side) / 2, width: side, height: side)

    let shadow = NSShadow()
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.3)
    shadow.shadowOffset = NSSize(width: 0, height: -canvas * 10 / 1024)
    shadow.shadowBlurRadius = canvas * 20 / 1024
    shadow.set()

    source.draw(in: rect, from: .zero, operation: .sourceOver, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()

    return rep.representation(using: .png, properties: [:])!
}

try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)

var images: [[String: String]] = []
for points in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let name = "icon_\(points)x\(points)\(scale == 2 ? "@2x" : "").png"
        try render(pixels: points * scale).write(to: output.appending(path: name))
        images.append(["idiom": "mac", "size": "\(points)x\(points)", "scale": "\(scale)x", "filename": name])
    }
}

let contents: [String: Any] = ["images": images, "info": ["author": "xcode", "version": 1]]
try JSONSerialization.data(withJSONObject: contents, options: [.prettyPrinted, .sortedKeys])
    .write(to: output.appending(path: "Contents.json"))
print("Wrote \(images.count) icons to \(output.path)")
