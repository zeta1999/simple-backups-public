import SwiftUI

@main
struct SimpleBackupsApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text("simple-backups")
                    .font(.largeTitle.weight(.semibold))
                Text("Pair a desktop repository, then sync snapshots. CLI is the supported path for v0; this shell is a placeholder for emulator work.")
                    .foregroundStyle(.secondary)
                Button("Pair (coming soon)") {}
                    .buttonStyle(.borderedProminent)
                Spacer()
            }
            .padding()
            .navigationTitle("Backups")
        }
    }
}

#Preview {
    ContentView()
}