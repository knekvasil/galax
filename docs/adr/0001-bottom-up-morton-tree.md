# ADR-0001: Bottom-up Morton tree construction

The FMM needs a spatial decomposition tree. A top-down recursive quadtree is the most common educational approach, but for production scale (10⁶+ bodies) a bottom-up construction via Morton-key sorting and prefix-based hierarchical grouping is used instead.

Bodies are sorted by Morton key, forming a Z-order curve that ensures spatial locality equals memory contiguity. Leaves are contiguous `[start, end)` ranges where each leaf contains at most `n_max` bodies. Parent cells are formed by merging adjacent sibling ranges upward, using common Morton prefix lengths to determine the hierarchy.

This avoids recursive traversal during construction, produces a cache-friendly contiguous node layout, and makes both interaction list computation and body range identification trivial — M2L partners are inferred directly from the sorted structure rather than expensive tree walking.
