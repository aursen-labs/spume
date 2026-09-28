package dev.spume.balance.core

import com.novi.serde.Bytes
import dev.spume.balance.HttpError
import dev.spume.balance.HttpHeader
import dev.spume.balance.HttpRequest
import dev.spume.balance.HttpResponse
import dev.spume.balance.HttpResult
import io.ktor.client.call.body
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.HttpRequestTimeoutException
import io.ktor.client.plugins.HttpTimeout
import io.ktor.client.request.headers
import io.ktor.client.request.request
import io.ktor.client.request.setBody
import io.ktor.http.HttpMethod
import io.ktor.util.flattenEntries
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import kotlin.coroutines.cancellation.CancellationException
import io.ktor.client.HttpClient as KtorHttpClient

class HttpClient {

    private val ktorHttpClient = KtorHttpClient(OkHttp) {
        install(HttpTimeout) {
            requestTimeoutMillis = 30000
            connectTimeoutMillis = 15000
            socketTimeoutMillis = 15000
        }
    }

    suspend fun request(request: HttpRequest): HttpResult = withContext(Dispatchers.Default) {
        try {
            HttpResult.Ok(requestResponse(request))
        } catch (ce: CancellationException) {
            throw ce
        } catch (error: Throwable) {
            HttpResult.Err(toHttpError(error))
        }
    }

    private suspend fun requestResponse(request: HttpRequest): HttpResponse {
        val response = ktorHttpClient.request(request.url) {
            this.method = HttpMethod.parse(request.method)
            this.headers {
                for (header in request.headers) {
                    append(header.name, header.value)
                }
            }
            if (request.body.content.isNotEmpty()) {
                setBody(request.body.content)
            }
        }

        val bytes: ByteArray = response.body()
        val headers = response.headers
            .flattenEntries()
            .map { HttpHeader(it.first, it.second) }
        return HttpResponse(response.status.value.toUShort(), headers, Bytes(bytes))
    }

    private fun toHttpError(error: Throwable): HttpError = when (error) {
        is HttpRequestTimeoutException, is SocketTimeoutException -> HttpError.Timeout
        is IllegalArgumentException -> HttpError.Url(error.message ?: "Invalid URL")
        is UnknownHostException -> HttpError.Io("Unknown host")
        else -> HttpError.Io(error.message ?: "HTTP request failed")
    }
}
