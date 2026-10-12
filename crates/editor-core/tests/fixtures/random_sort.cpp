// Generate random_sort.tsv using the MSVC v143 toolset. This mirrors the
// supplied Sorters.h's engine and shuffle, with a fixed seed for comparison.
#include <algorithm>
#include <cstdint>
#include <iostream>
#include <numeric>
#include <random>
#include <vector>

int main(int argc, char**) {
    if (argc > 1) {
        for (unsigned seed : {0u, 1u, 0xffffffffu}) {
            std::default_random_engine engine(seed);
            std::_Rng_from_urng<std::int64_t, std::default_random_engine> adapter(engine);
            for (std::int64_t bound : {1LL, 2LL, 2147483649LL, 4294967296LL,
                                      4294967297LL, 9223372036854775807LL}) {
                std::cout << seed << '\t' << bound << '\t';
                for (int i = 0; i < 32; ++i) {
                    if (i) std::cout << ',';
                    std::cout << adapter(bound);
                }
                std::cout << '\n';
            }
        }
        return 0;
    }
    for (unsigned seed : {0u, 1u, 5489u, 1791800000u, 0xffffffffu}) {
        for (size_t count : {0u, 1u, 2u, 3u, 5u, 17u, 625u, 1000u}) {
            std::vector<unsigned> items(count);
            std::iota(items.begin(), items.end(), 0u);
            std::shuffle(items.begin(), items.end(), std::default_random_engine(seed));
            std::cout << seed << '\t' << count << '\t';
            for (size_t i = 0; i < count; ++i) {
                if (i) std::cout << ',';
                std::cout << items[i];
            }
            std::cout << '\n';
        }
    }
}
