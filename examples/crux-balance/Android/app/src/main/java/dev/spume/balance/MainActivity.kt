package dev.spume.balance

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.spume.balance.core.Core

private const val DEFAULT_ADDRESS = "11111111111111111111111111111111"

class MainActivity : ComponentActivity() {
    private val core = Core()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        core.update(Event.GetBalance(DEFAULT_ADDRESS))

        setContent {
            MaterialTheme {
                val state by core.viewModel.collectAsState()
                var address by remember { mutableStateOf(DEFAULT_ADDRESS) }

                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.Center,
                        modifier = Modifier.padding(16.dp),
                    ) {
                        Text(
                            text = "Spume Balance Example",
                            fontSize = 30.sp,
                            modifier = Modifier.padding(10.dp)
                        )
                        Text(
                            text = "Rust Core, Kotlin Shell (Jetpack Compose)",
                            modifier = Modifier.padding(bottom = 16.dp)
                        )
                        OutlinedTextField(
                            value = address,
                            onValueChange = { address = it },
                            label = { Text("Address") },
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth()
                        )
                        Text(text = state.balance, modifier = Modifier.padding(16.dp))
                        Button(onClick = { core.update(Event.GetBalance(address)) }) {
                            Text(text = "Get balance")
                        }
                    }
                }
            }
        }
    }
}
