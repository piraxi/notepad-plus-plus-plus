// Independent fixture: execute the supplied Scintilla line buffer and markers.
#include <cstddef>
#include <cstdint>
#include <cassert>
#include <cstring>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>
#include <forward_list>
#include <optional>
#include <algorithm>
#include <memory>
#include <fstream>
#include <cstdio>
#include <cstdarg>
#include "ScintillaTypes.h"
#include "Debugging.h"
#include "Position.h"
#include "SplitVector.h"
#include "Partitioning.h"
#include "CellBuffer.h"
#include "PerLine.h"
using namespace Scintilla::Internal;
void Scintilla::Internal::Platform::DebugPrintf(const char *format, ...) noexcept {
    va_list args;
    va_start(args, format);
    vfprintf(stderr, format, args);
    va_end(args);
}

std::vector<std::string> words(int maximum) {
    std::vector<std::string> result{""};
    size_t first = 0;
    for (int length = 0; length < maximum; ++length) {
        const size_t end = result.size();
        for (size_t i = first; i < end; ++i)
            for (char c : {'a', '\r', '\n'}) result.push_back(result[i] + c);
        first = end;
    }
    return result;
}
std::string hex(const std::string &text) {
    std::string result;
    for (unsigned char c : text) {
        result += "0123456789abcdef"[c >> 4];
        result += "0123456789abcdef"[c & 15];
    }
    return result.empty() ? "-" : result;
}
int main(int argc, char **argv) {
    if (argc != 4) return 1;
    std::ofstream out(argv[1]);
    for (const auto &original : words(4))
    for (size_t start = 0; start <= original.size(); ++start)
    for (size_t end = start; end <= original.size(); ++end)
    for (const auto &replacement : words(2)) {
        if (start == end && replacement.empty()) continue;
        out << hex(original) << '\t' << start << '\t' << end << '\t' << hex(replacement);
        for (Sci::Line marked = 0; ; ++marked) {
            LineMarkers markers;
            CellBuffer buffer(false, false);
            buffer.SetPerLine(&markers);
            bool sequence = false;
            if (!original.empty()) buffer.InsertString(0, original.data(), original.size(), sequence);
            if (marked >= buffer.Lines()) break;
            markers.AddMark(marked, 1, buffer.Lines());
            if (end > start) buffer.DeleteChars(start, end - start, sequence);
            if (!replacement.empty()) buffer.InsertString(start, replacement.data(), replacement.size(), sequence);
            out << '\t' << markers.MarkerNext(0, 2);
        }
        out << '\n';
    }
    std::ofstream paste(argv[2]);
    for (const auto &original : words(4))
    for (const auto &replacement : words(2)) {
        CellBuffer initial(false, false);
        bool sequence = false;
        if (!original.empty()) initial.InsertString(0, original.data(), original.size(), sequence);
        for (unsigned mask = 0; mask < (1u << initial.Lines()); ++mask) {
            LineMarkers markers;
            CellBuffer buffer(false, false);
            buffer.SetPerLine(&markers);
            if (!original.empty()) buffer.InsertString(0, original.data(), original.size(), sequence);
            for (Sci::Line line = 0; line < buffer.Lines(); ++line)
                if (mask & (1u << line)) markers.AddMark(line, 1, buffer.Lines());
            // Notepad_plus::pasteToMarkedLines visits the current markers and
            // computes each target on the already edited buffer, bottom to top.
            for (Sci::Line line = buffer.Lines() - 1; line >= 0; --line) {
                if (!(markers.MarkValue(line) & 2)) continue;
                const auto start = buffer.LineStart(line);
                const auto end = buffer.LineEnd(line);
                if (end > start) buffer.DeleteChars(start, end - start, sequence);
                if (!replacement.empty()) buffer.InsertString(start, replacement.data(), replacement.size(), sequence);
            }
            std::string result(buffer.Length(), '\0');
            if (!result.empty()) buffer.GetCharRange(result.data(), 0, result.size());
            paste << hex(original) << '\t' << hex(replacement) << '\t' << mask << '\t' << hex(result);
            for (Sci::Line line = 0; line < buffer.Lines(); ++line)
                if (markers.MarkValue(line) & 2) paste << '\t' << line;
            paste << '\n';
        }
    }
    std::ofstream deletion(argv[3]);
    for (const auto &original : words(5)) {
        CellBuffer initial(false, false);
        bool sequence = false;
        if (!original.empty()) initial.InsertString(0, original.data(), original.size(), sequence);
        for (unsigned mask = 0; mask < (1u << initial.Lines()); ++mask)
        for (bool marked : {false, true}) {
            LineMarkers markers;
            CellBuffer buffer(false, false);
            buffer.SetPerLine(&markers);
            if (!original.empty()) buffer.InsertString(0, original.data(), original.size(), sequence);
            for (Sci::Line line = 0; line < buffer.Lines(); ++line)
                if (mask & (1u << line)) markers.AddMark(line, 1, buffer.Lines());
            std::string copied;
            for (Sci::Line line = buffer.Lines() - 1; line >= 0; --line) {
                if (((markers.MarkValue(line) & 2) != 0) != marked) continue;
                const auto start = buffer.LineStart(line);
                const auto end = line + 1 < buffer.Lines() ? buffer.LineStart(line + 1) : buffer.Length();
                std::string part(end - start, '\0');
                if (!part.empty()) buffer.GetCharRange(part.data(), start, part.size());
                copied = part + copied;
                markers.DeleteMark(line, 1, true);
                if (end > start) buffer.DeleteChars(start, end - start, sequence);
            }
            std::string result(buffer.Length(), '\0');
            if (!result.empty()) buffer.GetCharRange(result.data(), 0, result.size());
            deletion << hex(original) << '\t' << mask << '\t' << marked << '\t' << hex(result) << '\t' << hex(copied);
            for (Sci::Line line = 0; line < buffer.Lines(); ++line)
                if (markers.MarkValue(line) & 2) deletion << '\t' << line;
            deletion << '\n';
        }
    }
    return out.good() && paste.good() && deletion.good() ? 0 : 2;
}
