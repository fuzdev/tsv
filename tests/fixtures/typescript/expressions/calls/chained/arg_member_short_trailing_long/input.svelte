<script>
	// Member chain arg in chained call — short trailing members (.right.start) stay grouped
	function fn1() {
		for (const a of b) {
			if (c) {
				if (d) {
					if (e) {
						fn(
							a,
							obj
								.method(
									/** @type {number} */ /** @type {AssignmentExpression} */ assignment_in_labeled
										.right.start,
									/** @type {number} */ /** @type {AssignmentExpression} */ assignment_in_labeled
										.right.end,
								)
								.toString(),
						);
					}
				}
			}
		}
	}
</script>
