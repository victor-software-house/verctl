## demo 1.0.1

- Read every flag from the environment,
  so each long option reads `DEMO_<LONG>`.

  - An empty variable is unset.
  - A flag wins over its variable.

  ```sh
  DEMO_FORMAT=json demo status
  ```

  Precedence is flag, then variable, then default.
- Second.

