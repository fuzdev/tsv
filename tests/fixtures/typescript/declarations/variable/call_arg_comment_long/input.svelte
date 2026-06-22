<script>
	if (a) {
		if (b) {
			// 100 chars - stays inline (short arg with comment at boundary)
			const aaaaaaaaa1 = bbbbbbbbbbbbbbb.ccccccccc.ddddddddddd.eeeeeeeeee.fffffffff.ggg(/* x */ hh);

			// 101 chars - call args expand (short arg with comment exceeds boundary)
			const aaaaaaaaa2 = bbbbbbbbbbbbbbb.cccccccccc.ddddddddddd.eeeeeeeeee.fffffffff.ggg(
				/* x */ hh
			);

			// 101 chars - break after = (short arg without comment, poorly breakable)
			const aaaaaaaaa3 =
				bbbbbbbbbbbbbbbbbbb.cccccccccccc.ddddddddddddd.eeeeeeeeee.fffffffff.ggg(hh);
		}
	}
</script>
