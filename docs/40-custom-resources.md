# Custom resources

> This document will go through values that could be set on the custom resources
> managed by the operator.

## Organisation

In both custom resources, you will find a special field which is `organisation`.
This field is here to select on which organisation you wanna order managed
services.

It can be retrieve from the console directly in the organisation panel overview
in the top right corner or from the URL. It can have two forms, one starting by
`user_` and the other starting by `orga_` and in both cases following by a uuid.

## PostgreSql

Below, you will find the custom resource in yaml format that you can use to
deploy a managed services.

```yaml
---
apiVersion: api.clever-cloud.com/v1
kind: PostgreSql
metadata:
  namespace: default
  name: postgresql
spec:
  organisation: orga_xxxx
  options:
    version: 13
    encryption: false
  instance:
    region: par
    plan: s_sml
...
```

### Supported version

| Version | Code |
| ------- | ---- |
| `10`    | 10   |
| `11`    | 11   |
| `12`    | 12   |
| `13`    | 13   |
| `14`    | 14   |

### Supported region

For region, the code could be used to select the desired region.

| Name                             | Code     |
| -------------------------------- | -------- |
| Jeddah                           | `jed`    |
| Paris                            | `par`    |
| Singapore                        | `sgp`    |
| Montreal                         | `mtl`    |
| Roubaix                          | `rbx`    |
| Warsaw                           | `wsw`    |
| Roubaix (with hds certification) | `rbxhds` |
| Sydney                           | `syd`    |
| New Yord                         | `nyc`    |

### Supported plan

For plan, both name and code could be used to select the desired plan.

| Name                | Code       |
| ------------------- | ---------- |
| `DEV`               | `dev`      |
| `XXS Small Space`   | `xxs_sml`  |
| `XXS Medium Space`  | `xxs_med`  |
| `XXS Big Space`     | `xxs_big`  |
| `XS Tiny Space`     | `xs_tin`   |
| `XS Small Space`    | `xs_sml`   |
| `XS Medium Space`   | `xs_med`   |
| `XS Big Space`      | `xs_big`   |
| `S Small Space`     | `s_sml`    |
| `S Medium Space`    | `s_med`    |
| `S Big Space`       | `s_big`    |
| `S Huge Space`      | `s_hug`    |
| `M Small Space`     | `m_sml`    |
| `M Medium Space`    | `m_med`    |
| `M Big Space`       | `m_big`    |
| `L Small Space`     | `l_sml`    |
| `L Medium Space`    | `l_med`    |
| `L Big Space`       | `l_big`    |
| `XL Small Space`    | `xl_sml`   |
| `XL Medium Space`   | `xl_med`   |
| `XL Big Space`      | `xl_big`   |
| `XL Huge Space`     | `xl_hug`   |
| `XXL Small Space`   | `xxl_sml`  |
| `XXL Medium Space`  | `xxl_med`  |
| `XXL Big Space`     | `xxl_big`  |
| `XXL Huge Space`    | `xxl_hug`  |
| `XXXL Small Space`  | `xxxl_sml` |
| `XXXL Medium Space` | `xxxl_med` |
| `XXXL Big Space`    | `xxxl_big` |

## MySql

Below, you will find the custom resource in yaml format that you can use to
deploy a managed services.

```yaml
---
apiVersion: api.clever-cloud.com/v1
kind: MySql
metadata:
  namespace: default
  name: mysql
spec:
  organisation: orga_xxxx
  options:
    version: 80
    encryption: false
  instance:
    region: par
    plan: s_sml
...
```

### Supported version

| Version | Code |
| ------- | ---- |
| `5.7`   | 57   |
| `8.0`   | 80   |

### Supported region

For region, the code could be used to select the desired region.

| Name                             | Code     |
| -------------------------------- | -------- |
| Jeddah                           | `jed`    |
| Paris                            | `par`    |
| Singapore                        | `sgp`    |
| Montreal                         | `mtl`    |
| Roubaix                          | `rbx`    |
| Warsaw                           | `wsw`    |
| Roubaix (with hds certification) | `rbxhds` |
| Sydney                           | `syd`    |
| New Yord                         | `nyc`    |

### Supported plan

For plan, both name and code could be used to select the desired plan.

| Name               | Code      |
| ------------------ | --------- |
| `DEV`              | `dev`     |
| `XXS Small Space`  | `xxs_sml` |
| `XXS Medium Space` | `xxs_med` |
| `XXS Big Space`    | `xxs_big` |
| `XS Tiny Space`    | `xs_tin`  |
| `XS Small Space`   | `xs_sml`  |
| `XS Medium Space`  | `xs_med`  |
| `XS Big Space`     | `xs_big`  |
| `S Small Space`    | `s_sml`   |
| `S Medium Space`   | `s_med`   |
| `S Big Space`      | `s_big`   |
| `M Small Space`    | `m_sml`   |
| `M Medium Space`   | `m_med`   |
| `M Big Space`      | `m_big`   |
| `L Small Space`    | `l_sml`   |
| `L Medium Space`   | `l_med`   |
| `L Big Space`      | `l_big`   |
| `XL Small Space`   | `xl_sml`  |
| `XL Medium Space`  | `xl_med`  |
| `XL Big Space`     | `xl_big`  |
| `XXL Small Space`  | `xxl_sml` |
| `XXL Medium Space` | `xxl_med` |
| `XXL Big Space`    | `xxl_big` |
| `XXL Huge Space`   | `xxl_hug` |

## Redis

Below, you will find the custom resource in yaml format that you can use to
deploy a managed services.

```yaml
---
apiVersion: api.clever-cloud.com/v1
kind: Redis
metadata:
  namespace: default
  name: redis
spec:
  organisation: orga_xxxx
  options:
    version: 6010
    encryption: false
  instance:
    region: par
    plan: s_mono
...
```

### Supported version

| Version | Code |
| ------- | ---- |
| `6.2.6` | 626  |

### Supported region

For region, the code could be used to select the desired region.

| Name                             | Code     |
| -------------------------------- | -------- |
| Jeddah                           | `jed`    |
| Paris                            | `par`    |
| Singapore                        | `sgp`    |
| Montreal                         | `mtl`    |
| Roubaix                          | `rbx`    |
| Warsaw                           | `wsw`    |
| Roubaix (with hds certification) | `rbxhds` |
| Sydney                           | `syd`    |
| New Yord                         | `nyc`    |

### Supported plan

For plan, both name and code could be used to select the desired plan.

| Name  | Code         |
| ----- | ------------ |
| `S`   | `s_mono`     |
| `M`   | `m_mono`     |
| `L`   | `l_mono`     |
| `XL`  | `xl_mono`    |
| `2XL` | `xxl_mono`   |
| `3XL` | `xxxl_mono`  |
| `4XL` | `xxxxl_mono` |

## MongoDb

Below, you will find the custom resource in yaml format that you can use to
deploy a managed services.

```yaml
---
apiVersion: api.clever-cloud.com/v1
kind: MongoDb
metadata:
  namespace: default
  name: mongodb
spec:
  organisation: orga_xxxx
  options:
    version: 403
    encryption: false
  instance:
    region: par
    plan: xs_sml
...
```

### Supported version

| Version | Code |
| ------- | ---- |
| `4.0.3` | 403  |

### Supported region

For region, the code could be used to select the desired region.

| Name                             | Code     |
| -------------------------------- | -------- |
| Jeddah                           | `jed`    |
| Paris                            | `par`    |
| Singapore                        | `sgp`    |
| Montreal                         | `mtl`    |
| Roubaix                          | `rbx`    |
| Warsaw                           | `wsw`    |
| Roubaix (with hds certification) | `rbxhds` |
| Sydney                           | `syd`    |
| New Yord                         | `nyc`    |

### Supported plan

For plan, both name and code could be used to select the desired plan.

| Name               | Code      |
| ------------------ | --------- |
| `DEV`              | `dev`     |
| `XS Small Space`   | `xs_sml`  |
| `XS Medium Space`  | `xs_med`  |
| `XS Big Space`     | `xs_big`  |
| `S Small Space`    | `s_sml`   |
| `S Medium Space`   | `s_med`   |
| `S Big Space`      | `s_big`   |
| `M Small Space`    | `m_sml`   |
| `M Medium Space`   | `m_med`   |
| `M Big Space`      | `m_big`   |
| `M Huge Space`     | `m_hug`   |
| `L Small Space`    | `l_sml`   |
| `L Medium Space`   | `l_med`   |
| `L Big Space`      | `l_big`   |
| `XL Small Space`   | `xl_sml`  |
| `XL Medium Space`  | `xl_med`  |
| `XL Big Space`     | `xl_big`  |
| `XXL Small Space`  | `xxl_sml` |
| `XXL Medium Space` | `xxl_med` |
| `XXL Big Space`    | `xxl_big` |

## Pulsar

Below, you will find the custom resource in yaml format that you can use to
deploy a managed services.

```yaml
---
apiVersion: api.clever-cloud.com/v1beta1
kind: Pulsar
metadata:
  namespace: default
  name: pulsar
spec:
  organisation: orga_xxxx
  instance:
    region: par
...
```

Currently, the pulsar manages services is only available in the one region which
name is `par`. More will come, before the product will be generally available.

## ConfigProvider

Below, you will find the custom resource in yaml format that you can use to
provide extra configuration to your applications and managed services.

```yaml
---
apiVersion: api.clever-cloud.com/v1
kind: ConfigProvider
metadata:
  namespace: default
  name: config-provider
spec:
  organisation: orga_xxxx
  variables:
    REGION: par
...
```
