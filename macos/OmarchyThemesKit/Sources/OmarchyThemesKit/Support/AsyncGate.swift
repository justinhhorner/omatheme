/// Serializes async operations (like a semaphore of one). An actor alone wouldn't: it's
/// re-entrant across the backend calls an Apply awaits.
final class AsyncGate: Sendable {
    private let state = State()

    func withLock<T: Sendable>(_ body: () async throws -> T) async throws -> T {
        await state.acquire()
        do {
            let value = try await body()
            await state.release()
            return value
        } catch {
            await state.release()
            throw error
        }
    }

    private actor State {
        private var busy = false
        private var waiters: [CheckedContinuation<Void, Never>] = []

        func acquire() async {
            if busy {
                await withCheckedContinuation { waiters.append($0) }
            } else {
                busy = true
            }
        }

        func release() {
            if waiters.isEmpty {
                busy = false
            } else {
                waiters.removeFirst().resume()
            }
        }
    }
}
