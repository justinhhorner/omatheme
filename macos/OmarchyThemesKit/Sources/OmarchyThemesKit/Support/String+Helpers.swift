extension StringProtocol {
    /// "tokyo night" → "Tokyo night": only the first character changes.
    var uppercasingFirst: String {
        prefix(1).uppercased() + dropFirst()
    }
}
