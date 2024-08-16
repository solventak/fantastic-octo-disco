terraform {
    backend "s3" {
        bucket = "blockchain-explorer-terraform-state"
        region = "us-east-1"
        key = "tfstate"
    }
}

provider "aws" {
    region = "us-east-1"
}

resource "aws_eks_cluster" "blockchain-explorer" {
    name = "blockchain-explorer"
    role_arn = aws_iam_role.blockchain-explorer.arn

    vpc_config {
        subnet_ids = [
            aws_subnet.blockchain-explorer-subnet-1.id,
            aws_subnet.blockchain-explorer-subnet-2.id
        ]
    }

    depends_on = [
        aws_iam_role_policy_attachment.blockchain-explorer-AmazonEKSClusterPolicy
    ]
}

resource "aws_eks_node_group" "blockchain-explorer-basic" {
    cluster_name = aws_eks_cluster.blockchain-explorer.name
    node_group_name = "blockchain-explorer-basic"
    node_role_arn = aws_iam_role.blockchain-explorer.arn
    subnet_ids = [
        aws_subnet.blockchain-explorer-subnet-1.id,
        aws_subnet.blockchain-explorer-subnet-2.id
    ]

    scaling_config {
        desired_size = 2
        max_size = 2
        min_size = 1
    }

    update_config {
        max_unavailable_percentage = 100
    }

    depends_on = [
        aws_iam_role_policy_attachment.blockchain-explorer-AmazonEKSWorkerNodePolicy,
        aws_iam_role_policy_attachment.blockchain-explorer-AmazonEKS_CNI_Policy,
        aws_iam_role_policy_attachment.blockchain-explorer-AmazonEC2ContainerRegistryReadOnly
    ]
}

resource "aws_iam_role" "example" {
  name = "eks-node-group-example"

  assume_role_policy = data.aws_iam_policy_document.assume_role.json
}

resource "aws_iam_role_policy_attachment" "blockchain-explorer-AmazonEKSWorkerNodePolicy" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSWorkerNodePolicy"
  role       = aws_iam_role.blockchain-explorer.name
}

resource "aws_iam_role_policy_attachment" "blockchain-explorer-AmazonEKS_CNI_Policy" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKS_CNI_Policy"
  role       = aws_iam_role.blockchain-explorer.name
}

resource "aws_iam_role_policy_attachment" "blockchain-explorer-AmazonEC2ContainerRegistryReadOnly" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly"
  role       = aws_iam_role.blockchain-explorer.name
}


resource "aws_vpc" "blockchain-explorer" {
    cidr_block = "10.0.0.0/16"
    enable_dns_hostnames = true
}

resource "aws_subnet" "blockchain-explorer-subnet-1" {
    vpc_id = aws_vpc.blockchain-explorer.id
    cidr_block = "10.0.1.0/24"
    availability_zone = "us-east-1a"
    map_public_ip_on_launch = true
}

resource "aws_subnet" "blockchain-explorer-subnet-2" {
    vpc_id = aws_vpc.blockchain-explorer.id
    cidr_block = "10.0.2.0/24"
    availability_zone = "us-east-1b"
    map_public_ip_on_launch = true
}

data "aws_iam_policy_document" "assume_role" {
    statement {
        effect = "Allow"

        principals {
            type = "Service"
            identifiers = ["eks.amazonaws.com", "ec2.amazonaws.com"] // TODO: should we have two separate roles?
        }

        actions = ["sts:AssumeRole"]
    }
}

resource "aws_iam_role" "blockchain-explorer" {
    name = "blockchain-explorer"
    assume_role_policy = data.aws_iam_policy_document.assume_role.json
}

resource "aws_iam_role_policy_attachment" "blockchain-explorer-AmazonEKSClusterPolicy" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSClusterPolicy"
  role       = aws_iam_role.blockchain-explorer.name
}

resource "aws_route_table" "blockchain-explorer-public-rt" {
  vpc_id = aws_vpc.blockchain-explorer.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.blockchain-explorer-igw.id
  }

  tags = {
    Name = "blockchain-explorer-public-rt"
  }
}

resource "aws_internet_gateway" "blockchain-explorer-igw" {
  vpc_id = aws_vpc.blockchain-explorer.id

  tags = {
    Name = "blockchain-explorer-igw"
  }
}

resource "aws_route_table_association" "blockchain-explorer-subnet-1" {
  subnet_id      = aws_subnet.blockchain-explorer-subnet-1.id
  route_table_id = aws_route_table.blockchain-explorer-public-rt.id
}

resource "aws_route_table_association" "blockchain-explorer-subnet-2" {
  subnet_id      = aws_subnet.blockchain-explorer-subnet-2.id
  route_table_id = aws_route_table.blockchain-explorer-public-rt.id
}

