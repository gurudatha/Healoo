package com.healoo.app.realtime

import com.healoo.app.data.Message
import com.healoo.app.data.RealtimeEvent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.concurrent.TimeUnit

/**
 * Live channel to the Tokio/Tungstenite gateway (design doc 4.4): wss://<host>/v1/ws
 * Authenticated with the Auth0 access token in the upgrade request.
 * Server frames (JSON):
 *   {"type":"message.new","message":{...Message...}}
 *   {"type":"item.updated","item_id":"..."}
 *   {"type":"ping"}  -> the client answers {"type":"pong"}
 * Reconnects with exponential backoff (1 s … 30 s) until stop() is called.
 */
class RealtimeClient(
    baseUrl: String,
    private val http: OkHttpClient,
    private val token: () -> String?,
    private val json: Json,
    /** Renews the access token; called before reconnecting after the server closes with 4001. */
    private val refreshToken: suspend () -> String? = { null },
) {
    private val url = baseUrl.trimEnd('/').replaceFirst("https://", "wss://").replaceFirst("http://", "ws://") + "/v1/ws"
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _events = MutableSharedFlow<RealtimeEvent>(extraBufferCapacity = 64)
    val events: SharedFlow<RealtimeEvent> = _events

    private val wsClient by lazy { http.newBuilder().pingInterval(25, TimeUnit.SECONDS).readTimeout(0, TimeUnit.MILLISECONDS).build() }
    @Volatile private var socket: WebSocket? = null
    @Volatile private var running = false
    private var reconnectJob: Job? = null
    private var attempt = 0
    @Volatile private var tokenExpired = false

    fun start() {
        if (running) return
        running = true
        connect()
    }

    fun stop() {
        running = false
        reconnectJob?.cancel()
        socket?.close(1000, "bye")
        socket = null
    }

    private fun connect() {
        val t = token() ?: run { scheduleReconnect(); return }
        val req = Request.Builder().url(url).header("Authorization", "Bearer $t").build()
        socket = wsClient.newWebSocket(req, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                attempt = 0
                _events.tryEmit(RealtimeEvent.ConnectionChanged(true))
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                val frame = runCatching { json.parseToJsonElement(text).jsonObject }.getOrNull() ?: return
                when (frame["type"]?.jsonPrimitive?.content) {
                    "message.new" -> (frame["message"] as? JsonObject)?.let {
                        runCatching { json.decodeFromJsonElement(Message.serializer(), it) }.getOrNull()
                            ?.let { m -> _events.tryEmit(RealtimeEvent.NewMessage(m)) }
                    }
                    "item.updated" -> frame["item_id"]?.jsonPrimitive?.content?.let { _events.tryEmit(RealtimeEvent.ItemChanged(it)) }
                    "ping" -> webSocket.send("""{"type":"pong"}""")
                }
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                if (code == 4001) tokenExpired = true
                webSocket.close(code, null)
            }
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                if (code == 4001) tokenExpired = true
                dropped()
            }
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) = dropped()
        })
    }

    private fun dropped() {
        socket = null
        _events.tryEmit(RealtimeEvent.ConnectionChanged(false))
        scheduleReconnect()
    }

    private fun scheduleReconnect() {
        if (!running) return
        reconnectJob?.cancel()
        reconnectJob = scope.launch {
            val wait = (1000L shl attempt.coerceAtMost(5)).coerceAtMost(30_000L)
            attempt++
            delay(wait)
            if (tokenExpired) { tokenExpired = false; refreshToken() }
            if (running) connect()
        }
    }
}
