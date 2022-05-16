{{/*
Define the cleveroperator.namespace template if set with forceNamespace or .Release.Namespace is set
*/}}
{{- define "cleveroperator.namespace" -}}
{{- if .Values.forceNamespace -}}
{{ printf "namespace: %s" .Values.forceNamespace }}
{{- else -}}
{{ printf "namespace: %s" .Release.Namespace }}
{{- end -}}
{{- end -}}
