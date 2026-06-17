<script lang="ts">
// Non-hugging union (object + non-void member): object stays inline, keeps the comment
type A={/* c */ a:1}|boolean;

// Two object members: no hug, keeps the comment
type B={/* c */ a:1}|{b:2};

// Object + undefined (not a void type for hugging): keeps the comment
type C={/* c */ a:1}|undefined;

// Union breaks (too long) but the object body stays inline: keeps the comment
type D={/* c */ aaaaaaaaaaaaaaaaaaaa:1;bbbbbbbbbbbbbbbbbbbb:2;cccccccccccccccccccc:3}|boolean;
</script>
