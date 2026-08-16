package dev.spume.balance.core

import android.util.Log
import dev.spume.balance.CoreFfi
import dev.spume.balance.Effect
import dev.spume.balance.Event
import dev.spume.balance.Request
import dev.spume.balance.Requests
import dev.spume.balance.ViewModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class Core(private val httpClient: HttpClient = HttpClient()) {
    private val coreFfi = CoreFfi()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private val _viewModel: MutableStateFlow<ViewModel> = MutableStateFlow(getViewModel())
    val viewModel: StateFlow<ViewModel> = _viewModel.asStateFlow()

    fun update(event: Event) {
        Log.d(TAG, "update: $event")

        scope.launch {
            handleEffects(coreFfi.update(event.bincodeSerialize()))
        }
    }

    private suspend fun handleEffects(effects: ByteArray) {
        for (request in Requests.bincodeDeserialize(effects).value) {
            processRequest(request)
        }
    }

    private suspend fun processRequest(request: Request) {
        when (val effect = request.effect) {
            is Effect.Http -> {
                // The core already built the JSON-RPC body; just carry the bytes.
                val result = httpClient.request(effect.value)
                handleEffects(coreFfi.resolve(request.id, result.bincodeSerialize()))
            }

            is Effect.Render -> {
                _viewModel.value = getViewModel()
            }
        }
    }

    private fun getViewModel(): ViewModel = ViewModel.bincodeDeserialize(coreFfi.view())

    companion object {
        private const val TAG = "Core"
    }
}
