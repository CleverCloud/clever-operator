# Telemetry

> This document will go through telemetry information exposed by the operator.

## Metrics

This section explains metrics exposed by the application, part by part. Metrics
are exposed by default on `http://0.0.0.0:8000/metrics` using the prometheus
format.

### Clever-Cloud client related metrics

| name                                 | labels                                                          | kind    | description                                |
| ------------------------------------ | --------------------------------------------------------------- | ------- | ------------------------------------------ |
| clever_cloud_client_request          | endpoint: String, method: String, status: Integer               | Counter | number of request on clever cloud's apis   |
| clever_cloud_client_request_duration | endpoint: String, method: String, status: Integer, unit: String | Counter | duration of request on clever cloud's apis |

### Kubernetes client related metrics

| name                               | labels                                          | kind    | description                             |
| ---------------------------------- | ----------------------------------------------- | ------- | --------------------------------------- |
| kubernetes_client_request_success  | action: String, namespace: String               | Counter | number of successful kubernetes request |
| kubernetes_client_request_failure  | action: String, namespace: String               | Counter | number of failed kubernetes request     |
| kubernetes_client_request_duration | action: String, namespace: String, unit: String | Counter | duration of kubernetes request          |

### Operator reconciliation loop metrics

| name                                        | labels                                        | kind    | description                         |
| ------------------------------------------- | --------------------------------------------- | ------- | ----------------------------------- |
| kubernetes_operator_reconciliation_success  | kind: String                                  | Counter | number of successful reconciliation |
| kubernetes_operator_reconciliation_failed   | kind: String                                  | Counter | number of failed reconciliation     |
| kubernetes_operator_reconciliation_event    | kind: String, namespace: String, name: String | Counter | number of usert event               |
| kubernetes_operator_reconciliation_duration | kind: String, unit: String                    | Counter | duration of reconciliation          |

### Operator http server metrics

| name                                        | labels                                                      | kind    | description                                        |
| ------------------------------------------- | ----------------------------------------------------------- | ------- | -------------------------------------------------- |
| kubernetes_operator_server_request_success  | method: String, path: String, status: Integer               | Counter | number of successful request handled by the server |
| kubernetes_operator_server_request_failure  | method: String, path: String, status: Integer               | Counter | number of failed request handled by the server     |
| kubernetes_operator_server_request_duration | method: String, path: String, status: Integer, unit: String | Counter | duration of request handled by the server          |
