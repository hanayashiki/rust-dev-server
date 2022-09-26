(async () => {
  for (let i = 0; i < 1000; i++) {
    await fetch(
      "http://127.0.0.1:8000/node_modules/rxjs/src/internal/Observable.ts",
    );
  }
})();
