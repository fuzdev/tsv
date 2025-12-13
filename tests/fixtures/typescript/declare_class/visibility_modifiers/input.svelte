<script lang="ts">
	declare class WithVisibility {
		public pub: string;
		private priv: number;
		protected prot: boolean;
	}

	declare class MixedModifiers {
		public readonly pubRead: string;
		private static privStatic: number;
		protected readonly protRead: boolean;
	}

	declare class WithMethods {
		public method(): void;
		private helper(): string;
		protected internal(): number;
	}
</script>
