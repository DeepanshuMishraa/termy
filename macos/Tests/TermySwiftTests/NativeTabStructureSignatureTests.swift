import XCTest
@testable import TermySwift

/// Guards the rule behind the tab bar's animation gating: only structural
/// changes (count, order, selection, pin state) animate; title-only updates —
/// which programs emit many times a second — must not, or the bar churns.
final class NativeTabStructureSignatureTests: XCTestCase {
    private func signature(
        _ object: AnyObject,
        selected: Bool = false,
        pinned: Bool = false
    ) -> TabStructureSignature {
        TabStructureSignature(id: ObjectIdentifier(object), isSelected: selected, isPinned: pinned)
    }

    func testTitleOnlyChangeProducesEqualSignatures() {
        // `a`/`b` stand in for stable per-window tab identities, held for the
        // duration of the test so their ObjectIdentifiers stay valid.
        let a = NSObject()
        let b = NSObject()
        let before = [signature(a, selected: true), signature(b)]
        // A title-only update keeps id, selection, and pin state identical, so
        // the signature is unchanged and the refresh skips the spring.
        let after = [signature(a, selected: true), signature(b)]
        XCTAssertEqual(before, after)
    }

    func testSelectionChangeIsStructural() {
        let a = NSObject()
        let b = NSObject()
        let before = [signature(a, selected: true), signature(b, selected: false)]
        let after = [signature(a, selected: false), signature(b, selected: true)]
        XCTAssertNotEqual(before, after)
    }

    func testReorderIsStructural() {
        let a = NSObject()
        let b = NSObject()
        XCTAssertNotEqual([signature(a), signature(b)], [signature(b), signature(a)])
    }

    func testCountChangeIsStructural() {
        let a = NSObject()
        let b = NSObject()
        XCTAssertNotEqual([signature(a)], [signature(a), signature(b)])
    }

    func testPinChangeIsStructural() {
        let a = NSObject()
        XCTAssertNotEqual([signature(a, pinned: false)], [signature(a, pinned: true)])
    }
}
