package com.healoo.app.data

import java.util.concurrent.ConcurrentHashMap

/**
 * Unsent message text, kept while the app runs: leaving a discussion (or a person's Message sheet)
 * and coming back shows what was typed. Memory only, so closing the app clears everything.
 * Keys: "item:<itemId>" for a discussion, "person:<userId>" for a new message to a person.
 */
object Drafts {
    private val texts = ConcurrentHashMap<String, String>()

    operator fun get(key: String): String = texts[key].orEmpty()

    operator fun set(key: String, text: String) {
        if (text.isBlank()) texts.remove(key) else texts[key] = text
    }

    fun clear() = texts.clear()
}
