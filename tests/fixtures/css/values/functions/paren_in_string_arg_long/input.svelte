<style>
	div {
		/* 100 chars - stays inline (at boundary) */
		background: linear-gradient('(aaaaaaaaaaaaaa', #000 0%, #111 20%, #222 40%, #333 60%, #444 80%);

		/* 101 chars - the argument list breaks */
		background: linear-gradient(
			'(aaaaaaaaaaaaaaa',
			#000 0%,
			#111 20%,
			#222 40%,
			#333 60%,
			#444 80%
		);
	}
</style>
