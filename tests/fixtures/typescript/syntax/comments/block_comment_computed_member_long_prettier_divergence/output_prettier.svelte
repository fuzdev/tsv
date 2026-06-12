<script>
	// short - fits on one line
	const a = obj.aaa.bbb?.[/** @type {string} */ d];

	// 100 - optional computed, jsdoc cast stays inline
	const b =
		obj.aaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccc?.[/** @type {string} */ d];

	// 101 - optional computed, jsdoc cast breaks into brackets
	const c =
		/** @type {string} */ obj.aaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccc?.[
			d
		];

	// 100 - non-optional computed, jsdoc cast stays inline
	const e =
		obj.aaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccc[/** @type {string} */ d];

	// 101 - non-optional computed, jsdoc cast breaks into brackets
	const f =
		/** @type {string} */ obj.aaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccc[
			d
		];
</script>
