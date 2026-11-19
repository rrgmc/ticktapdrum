// Drives the app from outside, step by step, for tools/ios/drive.sh.
//
// The app draws everything with egui into one Metal view, so it has no accessibility elements.
// Taps and drags therefore use positions on the app's window, from 0 to 1 on each axis.
//
// The steps come from the environment variable DM_STEPS. xcodebuild passes TEST_RUNNER_DM_STEPS
// to the test runner under that name. Steps are separated by ";" and words by spaces.
import XCTest

final class Driver: XCTestCase {
    let app = XCUIApplication()
    // The app that taps, drags and typing go to: TickTapDrum, or the one `open` brought up.
    lazy var target = app

    func testDrive() throws {
        continueAfterFailure = false
        let steps = (ProcessInfo.processInfo.environment["DM_STEPS"] ?? "")
            .split(separator: ";")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
        if steps.isEmpty {
            XCTFail("DM_STEPS holds no steps")
        }
        for step in steps {
            print("DM step: \(step)")
            run(step)
        }
    }

    // Runs one step.
    private func run(_ step: String) {
        let words = step.split(separator: " ").map(String.init)
        let args = Array(words.dropFirst())
        // The text after the first word, for `type` and `note`.
        let rest = step.drop(while: { $0 != " " }).trimmingCharacters(in: .whitespaces)

        switch words[0] {
        case "launch":
            app.launch()
            target = app
        case "activate":
            app.activate()
            target = app
        case "terminate":
            app.terminate()
        case "open":
            // Brings another app to the front by its bundle id, such as the Files app.
            target = XCUIApplication(bundleIdentifier: rest)
            target.activate()
        case "tap":
            let n = numbers(args, count: 2, step: step)
            point(n[0], n[1]).tap()
        case "drag":
            let n = numbers(args, count: 4, step: step)
            point(n[0], n[1]).press(forDuration: 0.1, thenDragTo: point(n[2], n[3]))
        case "wait":
            let n = numbers(args, count: 1, step: step)
            Thread.sleep(forTimeInterval: n[0])
        case "shot":
            let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
            attachment.name = rest.isEmpty ? "shot" : rest
            attachment.lifetime = .keepAlways
            add(attachment)
        case "rotate":
            XCUIDevice.shared.orientation = orientation(rest, step: step)
        case "home":
            XCUIDevice.shared.press(.home)
        case "type":
            target.typeText(rest)
        case "note":
            print("DM note: \(rest)")
        default:
            XCTFail("unknown step: \(step)")
        }
    }

    // A point on the app's main window, or on the app itself when it shows no window.
    private func point(_ x: Double, _ y: Double) -> XCUICoordinate {
        let window = target.windows.firstMatch
        let base: XCUIElement = window.exists ? window : target
        return base.coordinate(withNormalizedOffset: CGVector(dx: x, dy: y))
    }

    // Reads exactly `count` numbers, or fails the test.
    private func numbers(_ args: [String], count: Int, step: String) -> [Double] {
        let values = args.compactMap(Double.init)
        if values.count != count || args.count != count {
            XCTFail("step needs \(count) numbers: \(step)")
        }
        return values
    }

    private func orientation(_ name: String, step: String) -> UIDeviceOrientation {
        switch name {
        case "portrait": return .portrait
        case "portraitUpsideDown": return .portraitUpsideDown
        case "landscapeLeft": return .landscapeLeft
        case "landscapeRight": return .landscapeRight
        default:
            XCTFail("unknown orientation: \(step)")
            return .portrait
        }
    }
}
