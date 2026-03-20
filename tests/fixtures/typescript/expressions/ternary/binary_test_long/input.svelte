<script>
	function fn() {
		if (cond) {
			// inline - binary condition fits on return line (100 chars)
			return aaaa.bbbbb.cccccccccccccccccccccc && (d.eee === 'ffffffffff' || d.eee === 'gggggggggg')
				? x.y(a, 'b')
				: x.z('c', d);

			// breaks - binary condition overflows, continuation indented (101 chars)
			return aaaa.bbbbb.ccccccccccccccccccccccc &&
				(d.eee === 'ffffffffff' || d.eee === 'gggggggggg')
				? x.y(a, 'b')
				: x.z('c', d);
		}
	}
</script>
