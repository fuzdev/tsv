<script lang="ts">
	// import.meta - module metadata
	const url=import.meta.url;const env=import.meta.env;

	// import.meta with property access
	const devMode=import.meta.env.DEV;

	// new.target - function construction metadata
	function Foo(){if(new.target){return new.target.prototype;}
	}

	// new.target in class constructor
	class Bar{constructor(){const target=new.target}
	}
</script>
