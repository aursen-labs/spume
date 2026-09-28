import App
import Foundation
import Shared

@MainActor
class Core: ObservableObject {
    @Published var view: ViewModel

    private var core: CoreFfi

    init() {
        self.core = CoreFfi()
        // swiftlint:disable:next force_try
        self.view = try! .bincodeDeserialize(input: [UInt8](core.view()))
    }

    func update(_ event: Event) {
        // swiftlint:disable:next force_try
        let effects = [UInt8](core.update(data: Data(try! event.bincodeSerialize())))

        // swiftlint:disable:next force_try
        let requests = try! Requests.bincodeDeserialize(input: effects).value
        for request in requests {
            processEffect(request)
        }
    }

    func processEffect(_ request: Request) {
        switch request.effect {
        case .render:
            DispatchQueue.main.async {
                // swiftlint:disable:next force_try
                self.view = try! .bincodeDeserialize(input: [UInt8](self.core.view()))
            }
        case .http(let httpRequest):
            Task {
                let result = await performHttpRequest(httpRequest)
                // swiftlint:disable force_try
                let effects = [UInt8](
                    self.core.resolve(
                        id: request.id,
                        data: Data(try! result.bincodeSerialize())
                    ))
                let requests = try! Requests.bincodeDeserialize(input: effects).value
                // swiftlint:enable force_try
                for request in requests {
                    self.processEffect(request)
                }
            }
        }
    }

    func performHttpRequest(_ request: HttpRequest) async -> HttpResult {
        guard let url = URL(string: request.url) else {
            return .err(.url("Invalid URL"))
        }

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = request.method
        for header in request.headers {
            urlRequest.addValue(header.value, forHTTPHeaderField: header.name)
        }
        // The JSON-RPC call the core built. `spume` assembled the body; the
        // shell just carries the bytes.
        urlRequest.httpBody = Data(request.body)

        do {
            let (data, response) = try await URLSession.shared.data(for: urlRequest)
            guard let httpResponse = response as? HTTPURLResponse else {
                return .err(.io("Not an HTTP response"))
            }
            let headers = (httpResponse.allHeaderFields as? [String: String] ?? [:])
                .map { HttpHeader(name: $0.key, value: $0.value) }
            return .ok(
                HttpResponse(
                    status: UInt16(httpResponse.statusCode),
                    headers: headers,
                    body: [UInt8](data)
                )
            )
        } catch {
            return .err(.io(error.localizedDescription))
        }
    }
}
