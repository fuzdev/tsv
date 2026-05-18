<script lang="ts">
	declare class A {
		get prop(): string;
		set prop(v: string);
	}

	declare class B {
		static get prop(): number;
		static set prop(v: number);
	}
</script>
