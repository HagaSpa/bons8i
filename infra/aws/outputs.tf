output "lambda_function_name" {
  value = aws_lambda_function.probe.function_name
}

output "schedule_name" {
  value = aws_scheduler_schedule.probe.name
}

output "sns_topic_arn" {
  value = aws_sns_topic.probe_errors.arn
}
