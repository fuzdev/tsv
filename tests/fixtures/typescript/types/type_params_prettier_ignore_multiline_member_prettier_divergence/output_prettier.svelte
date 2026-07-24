<script lang="ts">
	// a glued directive freezing a multi-line parameter expands the declaration
	// (tsv must-break); prettier instead keeps the `<…>` flat around the slice
	function fn1</* prettier-ignore */ T extends {
		x:   1
	}>(a: T): void {}

	// method host — class-member and interface-member signatures share the
	// always-inline declaration path, which expands the same way
	class C {
		fn2</* prettier-ignore */ T extends {
			x:   1
		}>(a: T): void {}
	}
</script>
