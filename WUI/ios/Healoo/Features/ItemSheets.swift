import SwiftUI

// Sheets used on the item screen and on New item: book an appointment, add an alert, move a
// visit, close an item. Recurrence follows DataItem_Design.md 3.7 (same rules as the server).

private let hhmm: DateFormatter = { let f = DateFormatter(); f.dateFormat = "HH:mm"; f.locale = Locale(identifier: "en_US_POSIX"); return f }()
private func timeText(_ d: Date) -> String { hhmm.string(from: d) }
private func timeDate(_ s: String) -> Date { hhmm.date(from: s).flatMap { t in
    Calendar.current.date(bySettingHour: Calendar.current.component(.hour, from: t), minute: Calendar.current.component(.minute, from: t), second: 0, of: Date())
} ?? Date() }

/// Repeat (frequency) and period chips, with the visit count or the reason it is not allowed.
struct RepeatPicker: View {
    let start: Date
    @Binding var frequency: Frequency?
    @Binding var period: Period?
    var noun = "visit"

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Repeat")
            FlowLayout(spacing: 8) {
                SageChip(label: "No repeat", selected: frequency == nil) { frequency = nil }
                ForEach(Frequency.allCases) { f in
                    SageChip(label: f.label, selected: frequency == f) {
                        frequency = f
                        if f == .daily && period == nil { period = .oneMonth }     // daily needs a period
                    }
                }
            }
            if let f = frequency {
                FieldLabel("For")
                FlowLayout(spacing: 8) {
                    ForEach(Period.allCases) { p in SageChip(label: p.label, selected: period == p) { period = p } }
                    if f != .daily { SageChip(label: "Until cancelled", selected: period == nil) { period = nil } }
                }
                let rule = Recurrence(frequency: f, period: period)
                let problem = RecurrenceRules.problem(start, rule)
                Text(problem ?? RecurrenceRules.visitCount(start, rule).map { n in
                    "\(n) \(noun)s, last on \(DateText.long(DateText.string(RecurrenceRules.nthDate(start, f, n - 1))))"
                } ?? "Repeats until cancelled.")
                .font(HFont.small).foregroundStyle(problem != nil ? Sage.clay : Sage.muted)
            }
        }
    }
}

/// Common shell: scrollable form, Cancel / confirm, shows the reason the action can't go ahead.
private struct FormSheet<Content: View>: View {
    let title: String
    let confirm: String
    let problem: String?
    let action: () async throws -> Void
    @ViewBuilder var content: () -> Content
    @Environment(\.dismiss) private var dismiss
    @State private var tried = false
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    content()
                    if tried, let problem { Text(problem).font(HFont.small).foregroundStyle(Sage.clay) }
                    if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
                }
                .padding(20)
            }
            .background(Sage.background)
            .navigationTitle(title).navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    if busy { ProgressView() } else {
                        Button(confirm) {
                            tried = true
                            guard problem == nil else { return }
                            busy = true; error = nil
                            Task {
                                do { try await action(); dismiss() }
                                catch { self.error = error.localizedDescription }
                                busy = false
                            }
                        }.fontWeight(.semibold)
                    }
                }
            }
        }
    }
}

private struct LabeledField<Content: View>: View {
    let label: String
    @ViewBuilder var content: () -> Content
    init(_ label: String, @ViewBuilder content: @escaping () -> Content) { self.label = label; self.content = content }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel(label)
            content().padding(.horizontal, 14).frame(minHeight: 48)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.field))
                .overlay(RoundedRectangle(cornerRadius: Radius.field).stroke(Sage.border))
        }
    }
}

// MARK: - Appointment

/// Form used by the booking sheet and by New item → Appointment.
struct AppointmentFields: View {
    let doctors: [UserProfile]
    @Binding var doctorId: String?
    @Binding var date: Date
    @Binding var time: Date
    @Binding var frequency: Frequency?
    @Binding var period: Period?
    @Binding var notes: String

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            FieldLabel("Doctor")
            if doctors.isEmpty {
                Text("Add a doctor to your contacts from Search first.").font(HFont.small).foregroundStyle(Sage.muted)
            }
            FlowLayout(spacing: 8) {
                ForEach(doctors) { d in SageChip(label: d.displayName, selected: doctorId == d.id) { doctorId = d.id } }
            }
            HStack(spacing: 12) {
                LabeledField("First visit") {
                    DatePicker("First visit", selection: $date, in: Calendar.current.startOfDay(for: Date())..., displayedComponents: .date)
                        .labelsHidden().tint(Sage.accent)
                }
                LabeledField("Time") { DatePicker("Time", selection: $time, displayedComponents: .hourAndMinute).labelsHidden().tint(Sage.accent) }
            }
            RepeatPicker(start: date, frequency: $frequency, period: $period)
            LabeledField("Notes (optional)") { TextField("Reason for the visit", text: $notes).font(HFont.body) }
        }
    }

    static func problem(doctorId: String?, date: Date, frequency: Frequency?, period: Period?) -> String? {
        if doctorId == nil { return "Choose a doctor." }
        return RecurrenceRules.problem(date, frequency.map { Recurrence(frequency: $0, period: period) })
    }

    static func build(patientId: String, doctorId: String, date: Date, time: Date, frequency: Frequency?, period: Period?, notes: String) -> NewAppointment {
        let n = notes.trimmingCharacters(in: .whitespacesAndNewlines)
        return NewAppointment(patientId: patientId, doctorId: doctorId, date: DateText.string(date), time: timeText(time),
                              notes: n.isEmpty ? nil : n, recurrence: frequency.map { Recurrence(frequency: $0, period: period) })
    }
}

struct BookAppointmentSheet: View {
    let patientId: String
    let doctors: [UserProfile]
    let onBook: (NewAppointment) async throws -> Void
    @State private var doctorId: String?
    @State private var date = Calendar.current.date(byAdding: .day, value: 1, to: Date())!
    @State private var time = timeDate("10:00")
    @State private var frequency: Frequency?
    @State private var period: Period?
    @State private var notes = ""

    init(patientId: String, doctors: [UserProfile], defaultDoctor: String?, onBook: @escaping (NewAppointment) async throws -> Void) {
        self.patientId = patientId; self.doctors = doctors; self.onBook = onBook
        _doctorId = State(initialValue: defaultDoctor ?? (doctors.count == 1 ? doctors[0].id : nil))
    }

    var body: some View {
        FormSheet(title: "Book appointment", confirm: "Book",
                  problem: AppointmentFields.problem(doctorId: doctorId, date: date, frequency: frequency, period: period)) {
            try await onBook(AppointmentFields.build(patientId: patientId, doctorId: doctorId!, date: date, time: time,
                                                     frequency: frequency, period: period, notes: notes))
        } content: {
            AppointmentFields(doctors: doctors, doctorId: $doctorId, date: $date, time: $time, frequency: $frequency, period: $period, notes: $notes)
        }
    }
}

// MARK: - Alert

let alertTypes: [(String, String)] = [("MEDICATION", "Medication"), ("FOLLOW_UP", "Follow-up"), ("CUSTOM", "Other")]

struct AlertFields: View {
    @Binding var type: String
    @Binding var text: String
    @Binding var date: Date
    @Binding var time: Date
    @Binding var frequency: Frequency?
    @Binding var period: Period?

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            FieldLabel("Type")
            FlowLayout(spacing: 8) {
                ForEach(alertTypes, id: \.0) { k, label in SageChip(label: label, selected: type == k) { type = k } }
            }
            LabeledField("Alert text") { TextField("e.g. Iron tablet after dinner", text: $text).font(HFont.body) }
            HStack(spacing: 12) {
                LabeledField("Starts") { DatePicker("Starts", selection: $date, displayedComponents: .date).labelsHidden().tint(Sage.accent) }
                LabeledField("Time") { DatePicker("Time", selection: $time, displayedComponents: .hourAndMinute).labelsHidden().tint(Sage.accent) }
            }
            RepeatPicker(start: date, frequency: $frequency, period: $period, noun: "alert")
        }
    }

    static func problem(text: String, date: Date, frequency: Frequency?, period: Period?) -> String? {
        if text.trimmingCharacters(in: .whitespaces).isEmpty { return "Write what the alert should say." }
        return RecurrenceRules.problem(date, frequency.map { Recurrence(frequency: $0, period: period) })
    }

    static func build(type: String, text: String, date: Date, time: Date, frequency: Frequency?, period: Period?) -> NewAlert {
        NewAlert(type: type, text: text.trimmingCharacters(in: .whitespacesAndNewlines), forUser: nil, date: DateText.string(date),
                 time: timeText(time), recurrence: frequency.map { Recurrence(frequency: $0, period: period) })
    }
}

struct AddAlertSheet: View {
    let onAdd: (NewAlert) async throws -> Void
    @State private var type = "MEDICATION"
    @State private var text = ""
    @State private var date = Date()
    @State private var time = timeDate("20:00")
    @State private var frequency: Frequency? = .daily
    @State private var period: Period? = .oneMonth

    var body: some View {
        FormSheet(title: "Add alert", confirm: "Add", problem: AlertFields.problem(text: text, date: date, frequency: frequency, period: period)) {
            try await onAdd(AlertFields.build(type: type, text: text, date: date, time: time, frequency: frequency, period: period))
        } content: {
            AlertFields(type: $type, text: $text, date: $date, time: $time, frequency: $frequency, period: $period)
        }
    }
}

// MARK: - Visits and closing

struct MoveVisitSheet: View {
    let visit: Visit
    let onMove: (_ date: String, _ time: String) async throws -> Void
    @State private var date: Date
    @State private var time: Date

    init(visit: Visit, onMove: @escaping (_ date: String, _ time: String) async throws -> Void) {
        self.visit = visit; self.onMove = onMove
        _date = State(initialValue: DateText.date(visit.date) ?? Date())
        _time = State(initialValue: timeDate(visit.time))
    }

    var body: some View {
        FormSheet(title: "Move this visit", confirm: "Move", problem: nil) {
            try await onMove(DateText.string(date), timeText(time))
        } content: {
            Text("Only this visit changes; the rest of the series stays the same.").font(HFont.small).foregroundStyle(Sage.muted)
            HStack(spacing: 12) {
                LabeledField("New date") {
                    DatePicker("New date", selection: $date, in: Calendar.current.startOfDay(for: Date())..., displayedComponents: .date)
                        .labelsHidden().tint(Sage.accent)
                }
                LabeledField("New time") { DatePicker("New time", selection: $time, displayedComponents: .hourAndMinute).labelsHidden().tint(Sage.accent) }
            }
        }
    }
}

/// Closing: feedback from anyone who closes; the star rating only when the patient closes (3.2).
struct CloseItemSheet: View {
    let canRate: Bool
    let onClose: (_ feedback: String?, _ rating: Int?) async throws -> Void
    @State private var feedback = ""
    @State private var rating = 0

    var body: some View {
        FormSheet(title: "Close this item", confirm: "Close item", problem: nil) {
            let f = feedback.trimmingCharacters(in: .whitespacesAndNewlines)
            try await onClose(f.isEmpty ? nil : f, canRate && rating > 0 ? rating : nil)
        } content: {
            Text("Future visits are cancelled and alerts stop. You can reopen it later.").font(HFont.small).foregroundStyle(Sage.muted)
            if canRate {
                FieldLabel("How was your care? (optional)")
                HStack(spacing: 4) {
                    ForEach(1...5, id: \.self) { n in
                        Button { rating = rating == n ? 0 : n } label: {
                            Image(systemName: n <= rating ? "star.fill" : "star").font(.system(size: 26)).foregroundStyle(Sage.sand)
                                .frame(width: 44, height: 44)
                        }
                        .accessibilityLabel("\(n) star\(n > 1 ? "s" : "")")
                        .accessibilityAddTraits(n == rating ? .isSelected : [])
                    }
                }
            }
            FieldLabel("Feedback (optional)")
            TextField("What went well, what could be better", text: $feedback, axis: .vertical)
                .lineLimit(3...6).font(HFont.body).padding(12)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.field))
                .overlay(RoundedRectangle(cornerRadius: Radius.field).stroke(Sage.border))
        }
    }
}
