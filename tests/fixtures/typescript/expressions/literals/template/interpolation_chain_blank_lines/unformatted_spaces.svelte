<script lang="ts">
  // Blank lines between chain members are preserved inside ${}
  const a = `prefix ${obj
      .method1()

      .method2()} suffix`;

  // Multiple blank lines in chain - all preserved
  const b = `${obj
      .method1()

      .method2()

      .method3()}`;

  // Blank line with optional chaining
  const c = `${obj
      ?.method1()

      ?.method2()}`;
</script>
