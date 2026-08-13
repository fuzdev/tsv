<script lang="ts">
	const a = 1;
	const b = 2;
	const c = 3;

	// blank after a head comment, before the first specifier
	export {
		// c1

		a
	};

	// blank before an own-line comment following a specifier
	export {
		b,

		// c2
		c
	};

	// re-export: blank after a head comment
	export {
		// c3

		d
	} from './d';
</script>
