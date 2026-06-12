<style>
  .parent   {
    color:   red;

    &   .a  ,
    &   .b   {
      color:   blue;
    }

    &:hover  ,
    &:focus   {
      background:   gray;
    }

    &   >   .c  ,
    &   +   .d  ,
    &   ~   .e   {
      margin:   0;
    }
  }
</style>
