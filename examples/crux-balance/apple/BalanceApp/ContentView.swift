import App
import SwiftUI

private let defaultAddress = "11111111111111111111111111111111"

struct ContentView: View {
    @ObservedObject var core: Core
    @State private var address = defaultAddress

    var body: some View {
        VStack {
            Text("Spume Balance Example")
                .font(.title)
                .padding()
            Text("Rust Core, Swift Shell (SwiftUI)")
                .padding(.bottom)

            TextField("Address", text: $address)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .padding(.horizontal)

            Text(core.view.balance)
                .font(.title2)
                .padding()

            ActionButton(label: "Get balance", color: .accentColor) {
                core.update(.getBalance(address))
            }
        }
        .onAppear {
            core.update(.getBalance(address))
        }
    }
}

struct ActionButton: View {
    var label: String
    var color: Color
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(label)
                .fontWeight(.bold)
                .font(.body)
                .padding(EdgeInsets(top: 10, leading: 15, bottom: 10, trailing: 15))
                .background(color)
                .cornerRadius(10)
                .foregroundColor(.white)
                .padding()
        }
    }
}

#Preview {
    ContentView(core: Core())
}
