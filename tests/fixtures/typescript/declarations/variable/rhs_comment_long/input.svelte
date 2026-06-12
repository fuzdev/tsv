<script>
	// Block comment on init - stays inline at exactly 100 visual chars
	const a = /** @type {AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA} */ bbbbbbbbb.cccccccc(-1);

	// Block comment on init - breaks after = at 101 visual chars
	const b =
		/** @type {AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA} */ bbbbbbbbb.cccccccc(-1);

	// Regular /* */ comment also breaks after = at 101 visual chars
	const c =
		/* AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA */ bbbbbbbbb.cccccccc(-1);

	// Block comment before member chain (no call) - breaks after =
	const d =
		/** @type {AAAAAAAAAAAAAAAAAAAAAAAAAAAAA} */ bbbbbbbbb.cccccccc.ddddddddddddddddddddddddddd;

	// Block comment before identifier - breaks after =
	const e =
		/** @type {AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA} */ bbbbbbbbbbbbbbbbbbb;
</script>
