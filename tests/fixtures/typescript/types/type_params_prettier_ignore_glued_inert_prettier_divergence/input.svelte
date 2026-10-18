<script lang="ts">
	// a glued directive is inert; the multi-line parameter expands the declaration
	// by ordinary layout
	function fn1<
		/* prettier-ignore */ T extends {
			x: 1;
		}
	>(a: T): void {}

	// method host: the same layout
	class C {
		fn2<
			/* prettier-ignore */ T extends {
				x: 1;
			}
		>(a: T): void {}
	}
</script>
