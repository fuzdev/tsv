<!-- declarations inside a nested style measure from their rendered column -->
<div>
	<style>
		.class {
			/* 100 columns: stays on one line */
			background: linear-gradient(0deg, rgba(0, 0, 0, 0.8) 0%, rgba(1, 1, 1, 0.8) 50%, var(--aaaa));
			/* 101 columns: the arguments break */
			background: linear-gradient(
				0deg,
				rgba(0, 0, 0, 0.8) 0%,
				rgba(1, 1, 1, 0.8) 50%,
				var(--aaaaa)
			);
		}
	</style>
</div>

<div>
	<section>
		<style>
			.class {
				/* 100 columns: stays on one line */
				background: linear-gradient(0deg, rgba(0, 0, 0, 0.8) 0%, rgba(1, 1, 1, 0.8) 50%, var(--aa));
				/* 101 columns: the arguments break */
				background: linear-gradient(
					0deg,
					rgba(0, 0, 0, 0.8) 0%,
					rgba(1, 1, 1, 0.8) 50%,
					var(--aaa)
				);
			}
		</style>
	</section>
</div>
