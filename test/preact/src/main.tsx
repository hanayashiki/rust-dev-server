import React from "preact/compat";

const App = () => {
  return <h1>Hello Preact</h1>;
};

React.render(<App />, document.getElementById("app")!);
