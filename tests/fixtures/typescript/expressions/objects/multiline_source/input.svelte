<script>
	/* 1. all inline - stays inline */
	const v1 = { outer: { inner: { deep: 1 } } };
	/* 2. newline after outermost { - outer expands, inner stays inline */
	const v2 = {
		outer: { inner: { deep: 1 } }
	};
	/* 3. newline after middle { - cascades: outer AND middle expand */
	const v3 = {
		outer: {
			inner: { deep: 1 }
		}
	};
	/* 4. newline after innermost { - cascades: ALL expand */
	const v4 = {
		outer: {
			inner: {
				deep: 1
			}
		}
	};
	/* 5. newline after outer + middle - same as just middle (cascading) */
	const v5 = {
		outer: {
			inner: { deep: 1 }
		}
	};
	/* 6. newline after all three - all expand */
	const v6 = {
		outer: {
			inner: {
				deep: 1
			}
		}
	};
	/* 7. newline before } only - collapses to inline */
	const v7 = { outer: { inner: { deep: 1 } } };
	/* 8. newline between properties - collapses to inline */
	const v8 = { a: 1, b: 2 };
	/* 9. blank line preserved in multiline */
	const v9 = {
		a: 1,

		b: 2
	};
</script>
