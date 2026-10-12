// Oracle built against the supplied Scintilla implementation, not the Rust generator.
#include <fstream>
#include <iomanip>
#include <string>
#include <string_view>
#include "../../../../notepad_plus_plus/scintilla/src/CaseConvert.h"
#include "../../../../notepad_plus_plus/scintilla/src/UniConversion.h"

int main(int argc, char **argv) {
    if (argc != 2) return 1;
    std::ofstream output(argv[1]);
    if (!output) return 2;
    using namespace Scintilla::Internal;
    for (int point = 0; point <= 0x10ffff; ++point) {
        if (point >= 0xd800 && point <= 0xdfff) continue;
        char input[5]{};
        UTF8FromUTF32Character(point, input);
        const size_t length = point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4;
        const std::string original(input, length);
        // Search's CaseFolderUnicode supplies spare output capacity. The string
        // convenience overload has a different capacity boundary for 3x folds.
        char converted[32]{};
        const size_t convertedLength = CaseConvertString(converted, sizeof(converted), input, length, CaseConversion::fold);
        if (!convertedLength) return 4;
        const std::string folded(converted, convertedLength);
        if (folded != original) {
            output << std::hex << point << '\t';
            for (unsigned char byte : folded) output << std::setw(2) << std::setfill('0') << static_cast<unsigned>(byte);
            output << '\n';
        }
    }
    return output ? 0 : 3;
}
