package com.healoo.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Vitals (Documentation/Vitals_Design.md). Not DataItems: small, frequent, per-person
// measurements with their own tables and API (/v1/vitals/...).
//
// Compatibility: every field the server may add later is ignored (ignoreUnknownKeys), every
// field it may stop sending has a default, and the colour is plain text rather than an enum so an
// unknown colour from a newer server can't fail decoding. The catalogue, default ranges and
// colour percentages come from the server (Server/config/vitals.json), never from the app.

/** Colour of a reading against the person's range. Unknown text reads as [GREEN]. */
enum class Severity(val label: String) {
    RED("Red"), YELLOW("Yellow"), GREEN("Green");
    companion object { fun of(text: String?) = entries.firstOrNull { it.name == text } ?: GREEN }
}

/** One number the person enters; most vitals have a single component called "value". */
@Serializable
data class VitalComponent(
    val key: String,
    val label: String = "",
    val low: Double? = null,        // sent to the person's doctor only
    val high: Double? = null,
)

/** One value as a percentage of another (urine output against water intake). Thresholds go to doctors only. */
@Serializable
data class VitalRatio(
    val numerator: String,
    val denominator: String,
    @SerialName("green_min_pct") val greenMinPct: Double? = null,
    @SerialName("yellow_min_pct") val yellowMinPct: Double? = null,
) {
    /** The numerator as a percentage of the denominator; null when it can't be worked out. */
    fun percent(values: Map<String, Double?>): Double? {
        val (n, d) = (values[numerator] ?: return null) to (values[denominator] ?: return null)
        return if (d > 0) n / d * 100 else null
    }
}

/** A vital as one person has it: built-in, or added for them by a doctor. */
@Serializable
data class VitalType(
    val code: String,
    val label: String = "",
    val icon: String = "other",
    val unit: String = "",
    val components: List<VitalComponent> = listOf(VitalComponent("value")),
    @SerialName("margin_pct") val marginPct: Int? = null,
    val custom: Boolean = false,
    val position: Int = 0,
    @SerialName("schema_version") val schemaVersion: Int = 1,
    /** Set for vitals coloured by one value as a percentage of another (urine). */
    val ratio: VitalRatio? = null,
)

@Serializable
data class VitalReading(
    @SerialName("reading_id") val id: String,
    @SerialName("user_id") val userId: String = "",
    val code: String,
    val values: Map<String, Double> = emptyMap(),
    @SerialName("taken_at") val takenAt: String = "",
    val note: String? = null,
    @SerialName("entered_by") val enteredBy: String = "",
    @SerialName("severity") val severityText: String = "GREEN",
    @SerialName("schema_version") val schemaVersion: Int = 1,
) {
    val severity: Severity get() = Severity.of(severityText)
}

/** One entry in a doctor's inbox: a patient's reading, with the doctor's own read state. */
@Serializable
data class VitalAlert(
    @SerialName("alert_id") val id: String,
    @SerialName("patient_id") val patientId: String,
    @SerialName("patient_name") val patientName: String = "",
    val code: String = "",
    val label: String = "",
    val icon: String = "other",
    val unit: String = "",
    val values: Map<String, Double> = emptyMap(),
    @SerialName("severity") val severityText: String = "GREEN",
    @SerialName("taken_at") val takenAt: String = "",
    @SerialName("schema_version") val schemaVersion: Int = 1,
) {
    val severity: Severity get() = Severity.of(severityText)
}

@Serializable
data class VitalAlertCounts(val red: Int = 0, val yellow: Int = 0, val green: Int = 0) {
    operator fun get(s: Severity) = when (s) { Severity.RED -> red; Severity.YELLOW -> yellow; Severity.GREEN -> green }
}

@Serializable
data class VitalAlertPage(val counts: VitalAlertCounts = VitalAlertCounts(), val data: List<VitalAlert> = emptyList())

/** What the apps need from the server's vitals configuration (GET /v1/vitals/config). */
@Serializable
data class VitalSettings(
    val icons: List<String> = listOf("other"),
    @SerialName("yellow_margin_pct") val yellowMarginPct: Int = 30,
    @SerialName("margin_pct_min") val marginPctMin: Int = 0,
    @SerialName("margin_pct_max") val marginPctMax: Int = 200,
    /** The vitals block on Home: tiles per row and rows (the last tile is More). */
    @SerialName("home_columns") val homeColumns: Int = 5,
    @SerialName("home_rows") val homeRows: Int = 2,
    /** How long ago a reading may have been taken when it is entered. */
    @SerialName("backdate_hours") val backdateHours: Int = 12,
)

/** [takenAt]: when it was measured (ISO time with offset); now if null. It can't be changed after saving. */
@Serializable
data class NewVitalReading(val code: String, val values: Map<String, Double>, val note: String? = null, @SerialName("taken_at") val takenAt: String? = null)

@Serializable
data class VitalBounds(val low: Double? = null, val high: Double? = null)

/** PUT /v1/vitals/ranges/{user}/{code}: a doctor's range for one patient. */
@Serializable
data class VitalRangeUpdate(val components: Map<String, VitalBounds>, @SerialName("margin_pct") val marginPct: Int? = null)

/** POST /v1/vitals/types: a doctor adds a vital for one patient. */
@Serializable
data class NewVitalType(
    @SerialName("user_id") val userId: String,
    val label: String,
    val icon: String,
    val unit: String = "",
    val low: Double? = null,
    val high: Double? = null,
    @SerialName("margin_pct") val marginPct: Int? = null,
)

/** How doctor alerts can be ordered. */
enum class AlertSort(val key: String, val label: String) { TIME("time", "Newest"), USER("user", "Patient"), VITAL("vital", "Vital") }

/** "120/80 mmHg", "98.6 °F": values in the vital's component order, without trailing ".0". */
fun formatVital(values: Map<String, Double>, unit: String, order: List<String> = emptyList()): String {
    val keys = order.filter { it in values } + values.keys.filter { it !in order }.sorted().let { rest ->
        // Blood pressure reads systolic/diastolic even when the type isn't at hand.
        if ("sys" in rest && "dia" in rest) listOf("sys", "dia") + (rest - setOf("sys", "dia")) else rest
    }
    val text = keys.joinToString("/") { k -> values.getValue(k).let { v -> if (v == Math.floor(v)) v.toLong().toString() else "%.1f".format(v) } }
    return if (unit.isBlank()) text else "$text $unit"
}
