<script lang="ts">
	/* short enum - multi-line formatting (default) */
	enum Short {
		X,
		Y,
		Z
	}

	/* long member names - still multi-line */
	enum LongMembers {
		AAAAAAAAAA,
		BBBBBBBBBB,
		CCCCCCCCCC,
		DDDDDDDDDD,
		EEEEEEEEEE,
		FFFFFFFFFF,
		GGGGGGGGGG
	}

	/* long string value - exceeds 100 chars, stays inline */
	enum LongString {
		X = 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'
	}

	/* long computed values - expressions wrap (exceeds 100 chars) */
	enum LongComputed {
		X = 1 +
			2 +
			3 +
			4 +
			5 +
			6 +
			7 +
			8 +
			9 +
			10 +
			11 +
			12 +
			13 +
			14 +
			15 +
			16 +
			17 +
			18 +
			19 +
			20 +
			21
	}
</script>
